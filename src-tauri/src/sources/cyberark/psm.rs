//! Connecting through CyberArk's session managers, the parts without a network: the login name PSM for
//! SSH takes, the start program PSM for Windows takes, and the RDP file PSMConnect hands out.

/// One SSH target as PSM for SSH (PSMP) is told about it in the login name.
#[derive(Default, Clone, Debug)]
pub struct PsmpTarget {
    /// The person's own CyberArk user. On Privilege Cloud (Shared Services) the full Identity login name,
    /// such as "john.smith@example.com" or "admin@cyberark.cloud.123456".
    pub vault_user: String,
    /// The account on the target, such as "root". Empty: PSMP asks for it in the terminal.
    pub target_user: String,
    /// The domain (or NIS domain, or SSH certificate group) the account belongs to, when it is not local.
    pub domain: String,
    pub target_address: String,
    /// 0 or 22: the default.
    pub target_port: u16,
    pub ticketing_system: String,
    pub ticket_id: String,
}

/// IPv6 addresses go in with hyphens instead of colons, as the PSMP syntax documents.
fn psmp_address(a: &str) -> String {
    let a = a.trim().trim_start_matches('[').trim_end_matches(']');
    if a.contains(':') { a.replace(':', "-") } else { a.to_string() }
}

/// The login name for PSM for SSH, without "@<PSMP address>" (that is the host the client connects to).
///
/// The short form is `<VaultUser>@<TargetUser>[#<Domain>]@<TargetAddress>[#<Port>]`. The vault user may
/// hold one "@"; anything else that does not fit the short form (a target user with "@", as a UPN,
/// no target user, a ticket) uses the documented key-value form
/// `+vu+<VaultUser>+tu+<TargetUser>[+da+<Domain>]+ta+<Address>[+tp+<Port>][+ti+<TicketID>][+ts+<System>]`.
pub fn psmp_user(t: &PsmpTarget) -> String {
    let vault = t.vault_user.trim();
    let mut user = t.target_user.trim().to_string();
    let mut domain = t.domain.trim().to_string();
    // "root#example.com" is how PSMP itself asks for a domain account.
    if domain.is_empty()
        && let Some((u, d)) = user.clone().split_once('#')
    {
        user = u.to_string();
        domain = d.to_string();
    }
    let address = psmp_address(&t.target_address);
    let port = if t.target_port == 0 || t.target_port == 22 { String::new() } else { t.target_port.to_string() };
    let ticket = !t.ticket_id.trim().is_empty();
    let short = !user.is_empty() && !user.contains('@') && vault.matches('@').count() <= 1 && !ticket;
    if short {
        let mut s = format!("{vault}@{user}");
        if !domain.is_empty() {
            s.push('#');
            s.push_str(&domain);
        }
        s.push('@');
        s.push_str(&address);
        if !port.is_empty() {
            s.push('#');
            s.push_str(&port);
        }
        return s;
    }
    let mut s = format!("+vu+{vault}");
    if !user.is_empty() {
        s.push_str(&format!("+tu+{user}"));
    }
    if !domain.is_empty() {
        s.push_str(&format!("+da+{domain}"));
    }
    s.push_str(&format!("+ta+{address}"));
    if !port.is_empty() {
        s.push_str(&format!("+tp+{port}"));
    }
    if ticket {
        s.push_str(&format!("+ti+{}", t.ticket_id.trim()));
        if !t.ticketing_system.trim().is_empty() {
            s.push_str(&format!("+ts+{}", t.ticketing_system.trim()));
        }
    }
    s
}

fn quoted(v: &str) -> String {
    if v.contains(' ') { format!("\"{v}\"") } else { v.to_string() }
}

/// The RDP start program ("alternate shell") for connecting with an RDP client through PSM for Windows:
/// `psm /u <target user> /a <target address> /c <connection component> [/ti <ticket> /ts <system>]`.
pub fn psm_start_program(target_user: &str, target_address: &str, component: &str, ticket_id: &str, ticketing_system: &str) -> String {
    let mut s = format!("psm /u {} /a {} /c {}", quoted(target_user.trim()), quoted(target_address.trim()), quoted(component.trim()));
    if !ticket_id.trim().is_empty() {
        s.push_str(&format!(" /ti {}", quoted(ticket_id.trim())));
        if !ticketing_system.trim().is_empty() {
            s.push_str(&format!(" /ts {}", quoted(ticketing_system.trim())));
        }
    }
    s
}

/// What Cablebag takes from an RDP file.
#[derive(Default, Debug, PartialEq)]
pub struct RdpFile {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub domain: String,
    pub alternate_shell: String,
    pub load_balance_info: String,
}

/// Reads the "name:type:value" lines of an RDP file. "full address" may carry the port; "server port"
/// says it too. An encrypted "password 51" is for the PC that made the file and is left out.
pub fn parse_rdp(text: &str) -> RdpFile {
    let mut f = RdpFile::default();
    for line in text.lines() {
        let line = line.trim_start_matches('\u{feff}').trim();
        let mut parts = line.splitn(3, ':');
        let (Some(name), Some(_kind), Some(value)) = (parts.next(), parts.next(), parts.next()) else { continue };
        let value = value.trim().to_string();
        match name.trim().to_lowercase().as_str() {
            "full address" => {
                let (h, p) = split_host_port(&value);
                f.host = h;
                if p != 0 {
                    f.port = p;
                }
            }
            "server port" => {
                if let Ok(p) = value.parse::<u16>() {
                    f.port = p;
                }
            }
            "username" => f.user = value,
            "domain" => f.domain = value,
            "alternate shell" => f.alternate_shell = value,
            "loadbalanceinfo" => f.load_balance_info = value,
            _ => {}
        }
    }
    f
}

/// "host:port", "[v6]:port", or a bare host (an IPv6 address without brackets has no port).
fn split_host_port(v: &str) -> (String, u16) {
    if let Some(rest) = v.strip_prefix('[')
        && let Some((h, p)) = rest.split_once(']')
    {
        return (h.to_string(), p.trim_start_matches(':').parse().unwrap_or(0));
    }
    match v.rsplit_once(':') {
        Some((h, p)) if !h.contains(':') => match p.parse::<u16>() {
            Ok(n) => (h.to_string(), n),
            Err(_) => (v.to_string(), 0),
        },
        _ => (v.to_string(), 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(vault: &str, user: &str, address: &str) -> PsmpTarget {
        PsmpTarget { vault_user: vault.into(), target_user: user.into(), target_address: address.into(), ..Default::default() }
    }

    #[test]
    fn psmp_login_names_as_documented() {
        // ssh john@root@target.ciscorouter.com@psmp.proxymachine.com
        assert_eq!(psmp_user(&t("john", "root", "target.ciscorouter.com")), "john@root@target.ciscorouter.com");
        // A port that is not 22 follows the address after "#".
        let mut x = t("john", "root", "target.ciscorouter.com");
        x.target_port = 2222;
        assert_eq!(psmp_user(&x), "john@root@target.ciscorouter.com#2222");
        x.target_port = 22;
        assert_eq!(psmp_user(&x), "john@root@target.ciscorouter.com");
        // ssh john@root#mycompany.com@target.mycompany.com@psmp.proxymachine.com
        let mut d = t("john", "root", "target.mycompany.com");
        d.domain = "mycompany.com".into();
        assert_eq!(psmp_user(&d), "john@root#mycompany.com@target.mycompany.com");
        assert_eq!(psmp_user(&t("john", "root#mycompany.com", "target.mycompany.com")), "john@root#mycompany.com@target.mycompany.com");
        // A vault user with one "@" (an LDAP UPN, or an Identity login name on Shared Services).
        assert_eq!(psmp_user(&t("john@mycompany.com", "localadmin", "targetmachine")), "john@mycompany.com@localadmin@targetmachine");
        assert_eq!(psmp_user(&t("admin@cyberark.cloud.106785", "root", "52.30.209.158")), "admin@cyberark.cloud.106785@root@52.30.209.158");
        // IPv6 with hyphens.
        assert_eq!(psmp_user(&t("john", "root", "fd00::5")), "john@root@fd00--5");
    }

    #[test]
    fn psmp_key_value_form_when_the_short_one_does_not_fit() {
        // A UPN target user: ssh +vu+<vaultUser>+tu+user@domain.com+ta+<targetAddress>@<ProxyAddress>
        assert_eq!(psmp_user(&t("john", "svc@example.com", "10.0.0.7")), "+vu+john+tu+svc@example.com+ta+10.0.0.7");
        // No target user: PSMP asks for it.
        assert_eq!(psmp_user(&t("john", "", "10.0.0.7")), "+vu+john+ta+10.0.0.7");
        // A ticket.
        let mut x = t("john", "root", "10.0.0.7");
        x.target_port = 2022;
        x.ticket_id = "INC123".into();
        x.ticketing_system = "ServiceNow".into();
        assert_eq!(psmp_user(&x), "+vu+john+tu+root+ta+10.0.0.7+tp+2022+ti+INC123+ts+ServiceNow");
    }

    #[test]
    fn psm_start_program_as_documented() {
        assert_eq!(psm_start_program("admin", "10.10.2.145", "PSM-RDP", "", ""), "psm /u admin /a 10.10.2.145 /c PSM-RDP");
        assert_eq!(psm_start_program("domainadmin@mycompany.com", "10.10.2.145", "PSM-RDP", "", ""), "psm /u domainadmin@mycompany.com /a 10.10.2.145 /c PSM-RDP");
        assert_eq!(psm_start_program("admin", "srv1", "PSM-RDP", "CHG1", "Ticketing one"), "psm /u admin /a srv1 /c PSM-RDP /ti CHG1 /ts \"Ticketing one\"");
    }

    #[test]
    fn psm_connect_rdp_file() {
        let f = parse_rdp(include_str!("../../../tests/cyberark/psm_connect.rdp"));
        assert_eq!(f.host, "psm.example.com");
        assert_eq!(f.port, 3390);
        assert_eq!(f.user, "jdoe");
        assert_eq!(f.domain, "");
        assert_eq!(f.alternate_shell, "psm /u admin /a 10.0.0.5 /c PSM-RDP /t 00000000-test-only");
        assert_eq!(f.load_balance_info, "tsv://MS Terminal Services Plugin.1.PSM_Farm");
        let bare = parse_rdp("full address:s:10.1.2.3\r\nusername:s:CORP\\jdoe\r\npassword 51:b:01000000D08C\r\n");
        assert_eq!((bare.host.as_str(), bare.port, bare.user.as_str()), ("10.1.2.3", 0, "CORP\\jdoe"));
        assert_eq!(split_host_port("[fd00::5]:3389"), ("fd00::5".to_string(), 3389));
        assert_eq!(split_host_port("fd00::5"), ("fd00::5".to_string(), 0));
    }
}
