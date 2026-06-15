//! Small workarounds for noisy third-party libraries.

/// Sends stderr to /dev/null until dropped, then puts it back.
///
/// The C audio stack (alsa/jack/pulse) behind cpal/rodio prints device-probe
/// errors straight to stderr, which garbles the TUI on a headless machine.
/// Redirecting the fd hides it whatever library is to blame, and restoring on
/// drop keeps the redirect scoped to the probe.
#[cfg(unix)]
pub struct StderrSilencer {
    saved_stderr: libc::c_int,
}

#[cfg(unix)]
impl StderrSilencer {
    pub fn new() -> Option<Self> {
        // SAFETY: plain fd calls; every fd opened here is closed or restored.
        unsafe {
            let devnull = libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY);
            if devnull < 0 {
                return None;
            }
            let saved_stderr = libc::dup(libc::STDERR_FILENO);
            if saved_stderr < 0 {
                libc::close(devnull);
                return None;
            }
            if libc::dup2(devnull, libc::STDERR_FILENO) < 0 {
                libc::close(devnull);
                libc::close(saved_stderr);
                return None;
            }
            libc::close(devnull);
            Some(Self { saved_stderr })
        }
    }
}

#[cfg(unix)]
impl Drop for StderrSilencer {
    fn drop(&mut self) {
        // SAFETY: saved_stderr is the fd we dup'd from stderr in `new`.
        unsafe {
            libc::dup2(self.saved_stderr, libc::STDERR_FILENO);
            libc::close(self.saved_stderr);
        }
    }
}

/// No-op elsewhere: no alsa/jack/pulse to quiet.
#[cfg(not(unix))]
pub struct StderrSilencer;

#[cfg(not(unix))]
impl StderrSilencer {
    pub fn new() -> Option<Self> {
        Some(Self)
    }
}
