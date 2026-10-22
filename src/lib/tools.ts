import { Waypoints, Radar, ScanSearch, Network, Power, KeyRound, ListChecks } from "@lucide/svelte";

export interface ToolInfo {
  id: string;
  name: string;
  description: string;
  icon: any;
}

export const TOOLS: ToolInfo[] = [
  { id: "tunnels", name: "Tunnels", description: "Port forwarding over SSH", icon: Waypoints },
  { id: "runmany", name: "Run on many servers", description: "One command on every server of a folder", icon: ListChecks },
  { id: "netscan", name: "Network scanner", description: "Find the computers on a network", icon: Radar },
  { id: "portscan", name: "Port scanner", description: "Find the open ports of one computer", icon: ScanSearch },
  { id: "openports", name: "Open ports on this PC", description: "Which programs listen and connect", icon: Network },
  { id: "wol", name: "Wake on LAN", description: "Switch on a computer over the network", icon: Power },
  { id: "keygen", name: "SSH key generator", description: "Create a key and put it on a server", icon: KeyRound },
];

export const toolInfo = (id: string) => TOOLS.find((t) => t.id === id) ?? TOOLS[0];

/** Well-known ports with a readable name, used by the scanners. */
export const KNOWN_PORTS: Record<number, string> = {
  20: "FTP data", 21: "FTP", 22: "SSH", 23: "Telnet", 25: "SMTP", 53: "DNS", 67: "DHCP", 69: "TFTP",
  80: "HTTP", 88: "Kerberos", 110: "POP3", 111: "RPC", 123: "NTP", 135: "Windows RPC", 137: "NetBIOS",
  139: "NetBIOS", 143: "IMAP", 161: "SNMP", 389: "LDAP", 443: "HTTPS", 445: "Windows file sharing",
  465: "SMTPS", 514: "Syslog", 587: "Mail submission", 631: "Printing", 636: "LDAPS", 873: "rsync",
  993: "IMAPS", 995: "POP3S", 1433: "SQL Server", 1521: "Oracle", 1883: "MQTT", 2049: "NFS", 2375: "Docker",
  2376: "Docker TLS", 3000: "Web app", 3128: "Proxy", 3306: "MySQL", 3389: "Remote Desktop", 5000: "Web app",
  5060: "SIP", 5432: "PostgreSQL", 5672: "AMQP", 5900: "VNC", 5985: "WinRM", 5986: "WinRM TLS", 6379: "Redis",
  6443: "Kubernetes", 8006: "Proxmox", 8080: "HTTP alternative", 8123: "Home Assistant", 8443: "HTTPS alternative",
  9000: "Web app", 9090: "Prometheus", 9100: "Printer / exporter", 9200: "Elasticsearch", 10050: "Zabbix agent",
  27017: "MongoDB",
};

export const COMMON_PORTS = Object.keys(KNOWN_PORTS).map(Number);
export const PROBE_PORTS = [22, 80, 443, 3389, 445, 5900, 8006, 23, 21, 8080];
