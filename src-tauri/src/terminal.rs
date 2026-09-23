use base64::Engine;
use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

struct TerminalSession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    job: Option<JobHandle>,
}

#[derive(Default)]
pub struct TerminalState(Mutex<Option<TerminalSession>>);

/// Puts a freshly spawned child (a login shell, which forks helper
/// processes of its own during startup — plain `child.kill()` only
/// touches the one process we have a handle for and leaves those
/// running) in a Windows Job Object, so the whole tree can be brought
/// down in one shot: explicitly via `JobHandle::terminate`, or — if we
/// never get the chance, e.g. the app itself is forcefully killed —
/// automatically, since Windows closes the job handle (triggering
/// KILL_ON_JOB_CLOSE) the moment *our* process exits for any reason.
#[cfg(windows)]
struct JobHandle(windows_sys::Win32::Foundation::HANDLE);

#[cfg(windows)]
unsafe impl Send for JobHandle {}

#[cfg(windows)]
impl JobHandle {
    fn terminate(&self) {
        use windows_sys::Win32::System::JobObjects::TerminateJobObject;
        unsafe {
            TerminateJobObject(self.0, 1);
        }
    }
}

#[cfg(windows)]
fn confine_to_job(child: &dyn Child) -> Option<JobHandle> {
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_BASIC_LIMIT_INFORMATION,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };

    let raw_handle = child.as_raw_handle()?;
    let process_handle = raw_handle as HANDLE;

    unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return None;
        }

        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation = JOBOBJECT_BASIC_LIMIT_INFORMATION {
            LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            ..std::mem::zeroed()
        };

        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const _,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );

        AssignProcessToJobObject(job, process_handle);
        Some(JobHandle(job))
    }
}

#[cfg(not(windows))]
struct JobHandle;

#[cfg(not(windows))]
impl JobHandle {
    fn terminate(&self) {}
}

#[cfg(not(windows))]
fn confine_to_job(_child: &dyn Child) -> Option<JobHandle> {
    None
}

/// Prefers Git Bash (what the app was asked for) if it's installed
/// alongside git.exe, falling back to PowerShell — every Windows install
/// has that, unlike bash.
fn resolve_shell() -> CommandBuilder {
    if let Ok(git_path) = which_git() {
        // git.exe's location varies (Git\cmd\git.exe, Git\mingw64\bin\git.exe,
        // ...) but bash.exe always lives at <GitRoot>\bin\bash.exe — walk up
        // looking for it instead of assuming a fixed number of hops.
        let mut dir = git_path.parent();
        for _ in 0..4 {
            let Some(d) = dir else { break };
            let candidate = d.join("bin").join("bash.exe");
            if candidate.exists() {
                let mut cmd = CommandBuilder::new(candidate);
                cmd.arg("--login");
                cmd.arg("-i");
                return cmd;
            }
            dir = d.parent();
        }
    }

    if cfg!(windows) {
        CommandBuilder::new("powershell.exe")
    } else {
        CommandBuilder::new(std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string()))
    }
}

fn which_git() -> std::io::Result<std::path::PathBuf> {
    let output = std::process::Command::new(if cfg!(windows) { "where" } else { "which" })
        .arg("git")
        .output()?;
    let first_line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if first_line.is_empty() {
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, "git not found"))
    } else {
        Ok(std::path::PathBuf::from(first_line))
    }
}

#[tauri::command]
pub fn terminal_start(
    app: AppHandle,
    path: String,
    cols: u16,
    rows: u16,
    state: tauri::State<TerminalState>,
) -> Result<(), String> {
    let mut guard = state.0.lock().unwrap();
    if let Some(mut existing) = guard.take() {
        terminate_session(&mut existing);
    }

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| e.to_string())?;

    let mut cmd = resolve_shell();
    cmd.cwd(&path);

    let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
    drop(pair.slave);
    let job = confine_to_job(child.as_ref());

    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

    let app_for_thread = app.clone();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let encoded = base64::engine::general_purpose::STANDARD.encode(&buf[..n]);
                    if app_for_thread.emit("pty-output", encoded).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = app_for_thread.emit("pty-closed", ());
    });

    *guard = Some(TerminalSession { master: pair.master, writer, child, job });
    Ok(())
}

#[tauri::command]
pub fn terminal_write(data: String, state: tauri::State<TerminalState>) -> Result<(), String> {
    let mut guard = state.0.lock().unwrap();
    let session = guard.as_mut().ok_or_else(|| "No hay una terminal activa".to_string())?;
    session.writer.write_all(data.as_bytes()).map_err(|e| e.to_string())?;
    session.writer.flush().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn terminal_resize(cols: u16, rows: u16, state: tauri::State<TerminalState>) -> Result<(), String> {
    let guard = state.0.lock().unwrap();
    let session = guard.as_ref().ok_or_else(|| "No hay una terminal activa".to_string())?;
    session
        .master
        .resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn terminal_stop(state: tauri::State<TerminalState>) -> Result<(), String> {
    kill_session(&state);
    Ok(())
}

/// Terminates the whole process tree for a session — the job first (which
/// takes any helper processes the shell forked with it), then the tracked
/// child itself as a fallback for platforms/cases without a job.
fn terminate_session(session: &mut TerminalSession) {
    if let Some(job) = &session.job {
        job.terminate();
    }
    let _ = session.child.kill();
}

/// Kills any active shell process. Called both by the explicit "stop"
/// command and on app exit, so closing Stash never leaves an orphaned
/// shell running.
pub fn kill_session(state: &TerminalState) {
    if let Some(mut session) = state.0.lock().unwrap().take() {
        terminate_session(&mut session);
    }
}
