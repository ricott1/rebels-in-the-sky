pub fn mute_stderr_spam() {
    // Capture raw ALSA errors that would otherwise be written to stderr
    // https://github.com/RustAudio/cpal/blob/3e73d824295c18373e794ef7bb8f53c58bd8af67/examples/enumerate.rs
    #[cfg(target_os = "linux")]
    let _silence_alsa_errors = alsa::Output::local_error_handler().ok();

    // See this issue from 2016 for context: https://github.com/jackaudio/jack2/issues/226
    // Even though rebels-in-the-sky doesn't directly use/depend on jack,
    // it may still get loaded by libasound.so - by loading it preemptively and
    // setting up a dummy error logger, the usage of libasound.so won't call
    // into the default write-to-stderr handler.
    #[cfg(target_os = "linux")]
    jack::set_logger(jack::LoggerType::None);
}
