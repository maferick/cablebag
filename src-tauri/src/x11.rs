//! X11 forwarding: windows of Linux programs started in an SSH session, shown on this PC.
//!
//! The X server is VcXsrv, a separate program that comes with Cablebag (the folder vcxsrv next to it, fetched by
//! scripts/xserver.mjs). It is started on demand, when the first window of any session arrives, on a free display
//! with a random cookie of its own, and stopped when the last session that used it closes.
//!
//! The SSH server only ever sees a fake cookie, a new one per session. Each X11 channel it opens must present that
//! cookie, which is then replaced by the real one before the connection reaches the X server (as OpenSSH does).

use crate::term::TermOut;
use anyhow::{anyhow, bail, Result};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

/// The only X authorisation protocol forwarded.
pub const PROTOCOL: &str = "MIT-MAGIC-COOKIE-1";

/// The X11 forwarding of one SSH session.
pub struct Forward {
    app: AppHandle,
    out: TermOut,
    fake: [u8; 16],
    lease: tokio::sync::Mutex<Option<Lease>>,
}

impl Forward {
    pub fn new(app: AppHandle, out: TermOut) -> Arc<Self> {
        Arc::new(Self { app, out, fake: rand::random(), lease: Default::default() })
    }

    /// The cookie the SSH server gives the programs it starts.
    pub fn fake_cookie(&self) -> String {
        hex::encode(self.fake)
    }

    /// The session closed: the X server stops when no other session uses it.
    pub async fn release(&self) {
        self.lease.lock().await.take();
    }

    /// The running X server (started for this session if needed): its port and real cookie.
    async fn server(&self) -> Result<(u16, [u8; 16])> {
        let mut lease = self.lease.lock().await;
        if let Some(l) = lease.as_ref().filter(|l| l.alive()) {
            return Ok((l.port, l.cookie));
        }
        let l = acquire(&self.app).await?;
        let found = (l.port, l.cookie);
        *lease = Some(l);
        Ok(found)
    }

    /// One X11 channel, opened by the SSH server for a program that shows a window.
    pub async fn serve(self: Arc<Self>, channel: russh::Channel<russh::client::Msg>) {
        let mut stream = channel.into_stream();
        let Ok(Some(mut setup)) = tokio::time::timeout(Duration::from_secs(30), read_setup(&mut stream)).await else { return };
        // Without the session's cookie the channel just closes.
        let Some(at) = cookie_at(&setup, &self.fake) else { return };
        let (port, cookie) = match self.server().await {
            Ok(s) => s,
            Err(e) => {
                self.out.error(&format!("Linux windows cannot be shown: {e}"));
                return;
            }
        };
        setup[at..at + cookie.len()].copy_from_slice(&cookie);
        let Ok(mut sock) = tokio::net::TcpStream::connect(("127.0.0.1", port)).await else { return };
        let _ = sock.set_nodelay(true);
        if sock.write_all(&setup).await.is_ok() {
            let _ = tokio::io::copy_bidirectional(&mut stream, &mut sock).await;
        }
    }
}

const fn pad4(n: usize) -> usize {
    (n + 3) & !3
}

/// The lengths of the authorisation name and data in the first 12 bytes an X client sends.
fn auth_lengths(head: &[u8]) -> Option<(usize, usize)> {
    let read = |i: usize| match head[0] {
        b'B' => Some(u16::from_be_bytes([head[i], head[i + 1]]) as usize),
        b'l' => Some(u16::from_le_bytes([head[i], head[i + 1]]) as usize),
        _ => None,
    };
    if head.len() < 12 {
        return None;
    }
    Some((read(6)?, read(8)?))
}

/// The connection setup an X client sends first, with its authorisation name and data.
async fn read_setup<R: AsyncRead + Unpin>(r: &mut R) -> Option<Vec<u8>> {
    let mut setup = vec![0u8; 12];
    r.read_exact(&mut setup).await.ok()?;
    let (name, data) = auth_lengths(&setup)?;
    setup.resize(12 + pad4(name) + pad4(data), 0);
    r.read_exact(&mut setup[12..]).await.ok()?;
    Some(setup)
}

/// Where the cookie sits in a connection setup, when it is the session's fake one.
fn cookie_at(setup: &[u8], fake: &[u8; 16]) -> Option<usize> {
    let (name, data) = auth_lengths(setup)?;
    let at = 12 + pad4(name);
    if setup.len() < at + data || &setup[12..12 + name] != PROTOCOL.as_bytes() || setup[at..at + data] != fake[..] {
        return None;
    }
    Some(at)
}

#[cfg_attr(not(windows), allow(dead_code))]
/// One entry of an X authority file: this cookie, for any address of the display.
fn xauth_entry(display: u16, cookie: &[u8; 16]) -> Vec<u8> {
    let mut out = 0xffffu16.to_be_bytes().to_vec(); // FamilyWild
    for field in [&b""[..], display.to_string().as_bytes(), PROTOCOL.as_bytes(), &cookie[..]] {
        out.extend_from_slice(&(field.len() as u16).to_be_bytes());
        out.extend_from_slice(field);
    }
    out
}

/// A session's share of the running X server.
struct Lease {
    run: u64,
    port: u16,
    cookie: [u8; 16],
}

impl Lease {
    /// Still the X server that runs (it may have been closed or have crashed since).
    fn alive(&self) -> bool {
        SERVER.lock().as_mut().is_some_and(|s| s.run == self.run && s.running())
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        let mut server = SERVER.lock();
        if let Some(s) = server.as_mut().filter(|s| s.run == self.run) {
            s.users = s.users.saturating_sub(1);
            if s.users == 0 {
                server.take();
            }
        }
    }
}

struct Server {
    run: u64,
    child: std::process::Child,
    port: u16,
    cookie: [u8; 16],
    users: usize,
    auth: PathBuf,
    /// Ends the X server together with Cablebag, even when Cablebag does not get to stop it.
    #[cfg(windows)]
    _job: Option<job::Job>,
}

impl Server {
    fn running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.auth);
    }
}

static SERVER: parking_lot::Mutex<Option<Server>> = parking_lot::Mutex::new(None);
/// One start at a time, so two sessions never start two servers.
static STARTING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
#[cfg_attr(not(windows), allow(dead_code))]
static RUNS: AtomicU64 = AtomicU64::new(1);

async fn acquire(app: &AppHandle) -> Result<Lease> {
    let _one = STARTING.lock().await;
    {
        let mut server = SERVER.lock();
        if let Some(s) = server.as_mut() {
            if s.running() {
                s.users += 1;
                return Ok(Lease { run: s.run, port: s.port, cookie: s.cookie });
            }
            server.take();
        }
    }
    let mut s = start(app).await?;
    s.users = 1;
    let lease = Lease { run: s.run, port: s.port, cookie: s.cookie };
    *SERVER.lock() = Some(s);
    Ok(lease)
}

#[cfg_attr(not(windows), allow(dead_code))]
fn server_path(app: &AppHandle) -> Result<PathBuf> {
    if let Ok(p) = std::env::var("CABLEBAG_XSERVER") {
        return Ok(PathBuf::from(p));
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../xserver/vcxsrv/vcxsrv.exe");
    let bundled = app.path().resource_dir().map(|d| d.join("vcxsrv").join("vcxsrv.exe")).unwrap_or_default();
    // Development builds use the unpacked copy, not the one the bundler left next to the exe.
    let order = if cfg!(debug_assertions) { [dev, bundled] } else { [bundled, dev] };
    order
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| anyhow!("the X server (vcxsrv) was not found next to Cablebag. Installing Cablebag again brings it back."))
}

#[cfg_attr(not(windows), allow(dead_code))]
/// A display whose port is free on every address (the X server listens on all of them).
fn free_display() -> Result<u16> {
    for display in 20..100u16 {
        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 6000 + display));
        // Bound only, not listening, so Windows does not ask about the firewall for Cablebag.
        let Ok(sock) = socket2::Socket::new(socket2::Domain::IPV4, socket2::Type::STREAM, None) else { continue };
        if sock.bind(&addr.into()).is_ok() {
            return Ok(display);
        }
    }
    bail!("no free display number between 20 and 99.")
}

#[cfg(not(windows))]
async fn start(_app: &AppHandle) -> Result<Server> {
    // shortcut: only the Windows version comes with an X server; elsewhere the system's own would be used.
    bail!("showing Linux windows needs the X server that comes with Cablebag for Windows.")
}

#[cfg(windows)]
async fn start(app: &AppHandle) -> Result<Server> {
    use std::os::windows::process::CommandExt;
    use std::time::Instant;
    let exe = server_path(app)?;
    let dir = std::env::temp_dir().join("cablebag-x11");
    std::fs::create_dir_all(&dir)?;
    let cookie: [u8; 16] = rand::random();
    for _ in 0..3 {
        let display = free_display()?;
        let auth = dir.join(format!("auth{display}"));
        let log = dir.join(format!("vcxsrv{display}.log"));
        // The X server changes to its own folder and takes no argument longer than 127 characters.
        if auth.as_os_str().len() > 127 || log.as_os_str().len() > 127 {
            bail!("the folder for temporary files has too long a name for the X server: {}", dir.display());
        }
        std::fs::write(&auth, xauth_entry(display, &cookie))?;
        let child = std::process::Command::new(&exe)
            .arg(format!(":{display}"))
            .args(["-multiwindow", "-wgl", "-notrayicon", "-silent-dup-error", "-listen", "tcp", "-auth"])
            .arg(&auth)
            .arg("-logfile")
            .arg(&log)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(0x0800_0000) // no console window
            .spawn()
            .map_err(|e| anyhow!("the X server could not be started: {e}"))?;
        let _job = job::Job::kill_on_close(&child);
        let port = 6000 + display;
        let mut s = Server { run: RUNS.fetch_add(1, Ordering::Relaxed), child, port, cookie, users: 0, auth, _job };
        let deadline = Instant::now() + Duration::from_secs(15);
        while s.running() {
            if tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
                return Ok(s);
            }
            if Instant::now() > deadline {
                bail!("the X server did not start within 15 seconds.");
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        // It stopped, for example because another program took the display's port meanwhile: next display.
    }
    bail!("the X server stopped right after starting; its log is in {}.", dir.display())
}

#[cfg(windows)]
mod job {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    /// A job that ends its process when the job's handle closes, also when Cablebag itself ends.
    pub struct Job(HANDLE);

    // The handle is only closed, from whichever thread drops it.
    unsafe impl Send for Job {}

    impl Job {
        pub fn kill_on_close(child: &std::process::Child) -> Option<Job> {
            unsafe {
                let job = Job(CreateJobObjectW(None, None).ok()?);
                let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const core::ffi::c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
                .ok()?;
                AssignProcessToJobObject(job.0, HANDLE(child.as_raw_handle())).ok()?;
                Some(job)
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(order: u8, name: &[u8], data: &[u8]) -> Vec<u8> {
        let u16b = |n: usize| if order == b'B' { (n as u16).to_be_bytes() } else { (n as u16).to_le_bytes() };
        let mut s = vec![order, 0];
        s.extend_from_slice(&u16b(11));
        s.extend_from_slice(&u16b(0));
        s.extend_from_slice(&u16b(name.len()));
        s.extend_from_slice(&u16b(data.len()));
        s.extend_from_slice(&[0, 0]);
        s.extend_from_slice(name);
        s.resize(12 + pad4(name.len()), 0);
        s.extend_from_slice(data);
        s.resize(12 + pad4(name.len()) + pad4(data.len()), 0);
        s
    }

    #[tokio::test]
    async fn finds_the_fake_cookie_in_both_byte_orders() {
        let fake = [7u8; 16];
        for order in [b'l', b'B'] {
            let s = setup(order, PROTOCOL.as_bytes(), &fake);
            let read = read_setup(&mut &s[..]).await.unwrap();
            assert_eq!(read, s);
            assert_eq!(cookie_at(&read, &fake), Some(12 + 20));
        }
    }

    #[test]
    fn refuses_other_cookies_and_protocols() {
        let fake = [7u8; 16];
        assert_eq!(cookie_at(&setup(b'l', PROTOCOL.as_bytes(), &[8u8; 16]), &fake), None);
        assert_eq!(cookie_at(&setup(b'l', b"XDM-AUTHORIZATION-1", &fake), &fake), None);
        assert_eq!(cookie_at(&setup(b'l', b"", b""), &fake), None);
        assert_eq!(cookie_at(&setup(b'x', PROTOCOL.as_bytes(), &fake), &fake), None);
    }

    #[test]
    fn writes_an_xauth_entry() {
        let e = xauth_entry(23, &[1u8; 16]);
        assert_eq!(&e[..4], &[0xff, 0xff, 0, 0]);
        assert_eq!(&e[4..8], &[0, 2, b'2', b'3']);
        assert_eq!(&e[8..10], &[0, 18]);
        assert_eq!(&e[10..28], PROTOCOL.as_bytes());
        assert_eq!(&e[28..30], &[0, 16]);
        assert_eq!(&e[30..], &[1u8; 16]);
    }
}
