//! Ports on 127.0.0.1 that only Cablebag may use. The WireGuard web proxy and local forwards lead into a
//! tunnel, so another program on this PC must not be able to borrow them. On every connection the program at
//! the other end is looked up (the socket table gives its process), and it is let in only when it is Cablebag
//! itself or was started by Cablebag, directly or further down: the web views (msedgewebview2.exe) and the RDP
//! helper. Programs started from one of Cablebag's own local terminals count as Cablebag's too.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// What a decision needs to know about one process.
#[derive(Clone, Copy, Debug)]
struct Proc {
    parent: Option<u32>,
    /// Seconds since 1970; 0 when it could not be read.
    start: u64,
}

/// Whether `pid` is `own` or started by it. `procs` gives the processes that run now. Windows keeps a parent's
/// number after the parent has ended, and the number can be handed to a new process later, so a "parent" that
/// started after its child is someone else and ends the chain.
fn descends(pid: u32, own: u32, procs: impl Fn(u32) -> Option<Proc>) -> bool {
    let mut at = pid;
    let Some(mut cur) = procs(at) else { return false };
    for _ in 0..64 {
        if at == own {
            return true;
        }
        let Some(parent) = cur.parent.filter(|p| *p != at && *p != 0) else { return false };
        let Some(up) = procs(parent) else { return false };
        if up.start != 0 && cur.start != 0 && up.start > cur.start {
            return false;
        }
        (at, cur) = (parent, up);
    }
    false
}

/// Decisions already made, by process number and start time (so a number used again by a new process is not
/// mistaken for the old one).
struct Cache {
    map: HashMap<u32, (u64, bool)>,
}

impl Cache {
    const MAX: usize = 512;

    fn get(&self, pid: u32, start: u64) -> Option<bool> {
        self.map.get(&pid).filter(|(s, _)| *s == start).map(|(_, ok)| *ok)
    }

    fn put(&mut self, pid: u32, start: u64, ok: bool) {
        if self.map.len() >= Self::MAX {
            self.map.clear();
        }
        self.map.insert(pid, (start, ok));
    }
}

static CACHE: LazyLock<Mutex<Cache>> = LazyLock::new(|| Mutex::new(Cache { map: HashMap::new() }));
static REFUSED: AtomicU64 = AtomicU64::new(0);

fn proc_of(sys: &System, pid: u32) -> Option<Proc> {
    sys.process(Pid::from_u32(pid)).map(|p| Proc { parent: p.parent().map(|p| p.as_u32()), start: p.start_time() })
}

/// When the process `pid` started, to tell it apart from an earlier one with the same number. Cheap, so a
/// decision already made costs almost nothing.
#[cfg(windows)]
fn born(pid: u32) -> Option<u64> {
    use windows::Win32::Foundation::{CloseHandle, FILETIME};
    use windows::Win32::System::Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let (mut created, mut exited, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default(), FILETIME::default());
        let r = GetProcessTimes(h, &mut created, &mut exited, &mut kernel, &mut user);
        let _ = CloseHandle(h);
        r.ok()?;
        Some(((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64)
    }
}

#[cfg(not(windows))]
fn born(pid: u32) -> Option<u64> {
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::Some(&[Pid::from_u32(pid)]), true, ProcessRefreshKind::nothing());
    proc_of(&sys, pid).map(|p| p.start)
}

/// Whether the process `pid` may use Cablebag's ports.
fn allowed(pid: u32) -> bool {
    let own = std::process::id();
    if pid == own {
        return true;
    }
    let born = born(pid);
    if let Some(ok) = born.and_then(|b| CACHE.lock().get(pid, b)) {
        return ok;
    }
    let mut sys = System::new();
    sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let ok = descends(pid, own, |p| proc_of(&sys, p));
    if let Some(b) = born {
        CACHE.lock().put(pid, b, ok);
    }
    ok
}

/// The process at the other end of a connection to our `port` from `peer`, from the socket table.
fn owner(peer: SocketAddr, port: u16) -> Option<u32> {
    use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo};
    let family = if peer.is_ipv4() { AddressFamilyFlags::IPV4 } else { AddressFamilyFlags::IPV6 };
    let sockets = get_sockets_info(family, ProtocolFlags::TCP).ok()?;
    sockets.into_iter().find_map(|s| match s.protocol_socket_info {
        ProtocolSocketInfo::Tcp(t) if t.local_addr == peer.ip() && t.local_port == peer.port() && t.remote_port == port => {
            s.associated_pids.first().copied()
        }
        _ => None,
    })
}

/// Whether a connection that came in on our `port` from `peer` is Cablebag's own. Others are counted, nothing
/// else about them is kept.
pub async fn is_ours(peer: SocketAddr, port: u16) -> bool {
    let ok = tokio::task::spawn_blocking(move || owner(peer, port).is_some_and(allowed)).await.unwrap_or(false);
    if !ok {
        let n = REFUSED.fetch_add(1, Ordering::Relaxed) + 1;
        if n.is_power_of_two() {
            eprintln!("refused {n} connection(s) from other programs to Cablebag's WireGuard ports");
        }
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(rows: &[(u32, Option<u32>, u64)]) -> impl Fn(u32) -> Option<Proc> + '_ {
        move |pid| rows.iter().find(|r| r.0 == pid).map(|r| Proc { parent: r.1, start: r.2 })
    }

    #[test]
    fn cablebag_and_what_it_started_are_let_in() {
        // 100 Cablebag, 200 its web view, 300 the web view's network process, 400 the RDP helper.
        let t = [(1, None, 1), (100, Some(1), 10), (200, Some(100), 20), (300, Some(200), 21), (400, Some(100), 30)];
        for pid in [100, 200, 300, 400] {
            assert!(descends(pid, 100, table(&t)), "{pid}");
        }
    }

    #[test]
    fn other_programs_are_not() {
        // 500 a browser started by Explorer (2), 600 its child, 1 the system.
        let t = [(1, None, 1), (2, Some(1), 2), (100, Some(2), 10), (500, Some(2), 40), (600, Some(500), 41)];
        assert!(!descends(500, 100, table(&t)));
        assert!(!descends(600, 100, table(&t)));
        assert!(!descends(2, 100, table(&t)), "Cablebag's own parent is not Cablebag");
        assert!(!descends(999, 100, table(&t)), "a process that is gone");
    }

    #[test]
    fn a_parent_number_used_again_does_not_count() {
        // 700 was started by a program that ended long ago; its number went to Cablebag later.
        let t = [(100, None, 50), (700, Some(100), 20)];
        assert!(!descends(700, 100, table(&t)));
        // Started in the same second is still fine.
        let t = [(100, None, 50), (700, Some(100), 50)];
        assert!(descends(700, 100, table(&t)));
    }

    #[test]
    fn loops_and_missing_parents_end() {
        let t = [(10, Some(20), 5), (20, Some(10), 5), (30, Some(30), 5), (40, Some(41), 5)];
        assert!(!descends(10, 100, table(&t)));
        assert!(!descends(30, 100, table(&t)));
        assert!(!descends(40, 100, table(&t)));
    }

    #[test]
    fn the_cache_knows_a_process_by_its_start_time() {
        let mut c = Cache { map: HashMap::new() };
        c.put(42, 1000, true);
        c.put(43, 1000, false);
        assert_eq!(c.get(42, 1000), Some(true));
        assert_eq!(c.get(43, 1000), Some(false));
        assert_eq!(c.get(42, 2000), None, "the same number, a new process");
        assert_eq!(c.get(44, 1000), None);
        for pid in 0..Cache::MAX as u32 + 1 {
            c.put(1000 + pid, 1, true);
        }
        assert!(c.map.len() <= Cache::MAX);
    }

    #[test]
    fn this_process_and_its_children_are_ours() {
        assert!(allowed(std::process::id()));
        let mut child = if cfg!(windows) {
            std::process::Command::new("cmd").args(["/c", "ping -n 4 127.0.0.1 >nul"]).spawn().unwrap()
        } else {
            std::process::Command::new("sleep").arg("3").spawn().unwrap()
        };
        let ok = allowed(child.id());
        let _ = child.kill();
        assert!(ok, "a child of this process");
        // Our own parent (cargo, a shell) did not come from us.
        let mut sys = System::new();
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
        if let Some(parent) = proc_of(&sys, std::process::id()).and_then(|p| p.parent) {
            assert!(!allowed(parent));
        }
    }

    #[tokio::test]
    async fn a_connection_from_this_process_is_ours() {
        let l = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = l.local_addr().unwrap().port();
        let _c = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let (_s, peer) = l.accept().await.unwrap();
        assert_eq!(owner(peer, port), Some(std::process::id()));
        assert!(is_ours(peer, port).await);
    }
}
