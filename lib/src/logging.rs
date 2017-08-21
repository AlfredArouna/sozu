use libc;
use std::str::FromStr;
use std::cmp::{self,Ord};
use std::sync::Mutex;
use std::fmt::{Arguments,format};
use std::io::{stdout,Stdout,Write};
use std::ascii::AsciiExt;
use std::net::{SocketAddr,UdpSocket};
use std::net::TcpStream;
use mio_uds::UnixDatagram;


lazy_static! {
  pub static ref LOGGER: Mutex<Logger> = Mutex::new(Logger::new());
  pub static ref PID:    i32           = unsafe { libc::getpid() };
  pub static ref TAG:    String        = LOGGER.lock().unwrap().tag.clone();
}


pub struct Logger {
  pub directives: Vec<LogDirective>,
  pub backend:    LoggerBackend,
  pub tag:        String,
}

impl Logger {
  pub fn new() -> Logger {
    Logger {
      directives: vec!(LogDirective {
        name:  None,
        level: LogLevelFilter::Error,
      }),
      backend: LoggerBackend::Stdout(stdout()),
      tag:     "WAAAAAAAH".to_string()
    }
  }

  pub fn init(tag: String, spec: &str, backend: LoggerBackend) {
    let directives = parse_logging_spec(spec);
    if let Ok(ref mut logger) = LOGGER.lock() {
      logger.set_directives(directives);
      logger.backend = backend;
      logger.tag     = tag;
    }
    //trying to init the logger tag
    let ref t = *TAG;
  }

  pub fn log<'a>(&mut self, meta: &LogMetadata, args: Arguments) {
    if self.enabled(meta) {
      match self.backend {
        LoggerBackend::Stdout(ref mut stdout) => {
          stdout.write_fmt(args);
        },
        //FIXME: should have a buffer to write to instead of allocating a string
        LoggerBackend::Unix(ref mut socket) => {
          socket.send(format(args).as_bytes());
        },
        //FIXME: should have a buffer to write to instead of allocating a string
        LoggerBackend::Udp(ref mut socket, ref address) => {
          socket.send_to(format(args).as_bytes(), address);
        }
        LoggerBackend::Tcp(ref mut socket) => {
          socket.write_fmt(args);
        },
      }
    }
  }

  pub fn set_directives(&mut self, directives: Vec<LogDirective>) {
    self.directives = directives;
  }

  fn enabled(&self, meta: &LogMetadata) -> bool {
    // Search for the longest match, the vector is assumed to be pre-sorted.
    for directive in self.directives.iter().rev() {
      match directive.name {
        Some(ref name) if !meta.target.starts_with(&**name) => {},
        Some(..) | None => {
          return meta.level <= directive.level
        }
      }
    }
    false
  }
}

pub enum LoggerBackend {
  Stdout(Stdout),
  Unix(UnixDatagram),
  Udp(UdpSocket, SocketAddr),
  Tcp(TcpStream)
}

#[repr(usize)]
#[derive(Copy, Eq, Debug)]
pub enum LogLevel {
    /// The "error" level.
    ///
    /// Designates very serious errors.
    Error = 1, // This way these line up with the discriminants for LogLevelFilter below
    /// The "warn" level.
    ///
    /// Designates hazardous situations.
    Warn,
    /// The "info" level.
    ///
    /// Designates useful information.
    Info,
    /// The "debug" level.
    ///
    /// Designates lower priority information.
    Debug,
    /// The "trace" level.
    ///
    /// Designates very low priority, often extremely verbose, information.
    Trace,
}

static LOG_LEVEL_NAMES: [&'static str; 6] = ["OFF", "ERROR", "WARN", "INFO",
                                             "DEBUG", "TRACE"];

impl Clone for LogLevel {
    #[inline]
    fn clone(&self) -> LogLevel {
        *self
    }
}

impl PartialEq for LogLevel {
    #[inline]
    fn eq(&self, other: &LogLevel) -> bool {
        *self as usize == *other as usize
    }
}

impl PartialEq<LogLevelFilter> for LogLevel {
    #[inline]
    fn eq(&self, other: &LogLevelFilter) -> bool {
        *self as usize == *other as usize
    }
}

impl PartialOrd for LogLevel {
    #[inline]
    fn partial_cmp(&self, other: &LogLevel) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialOrd<LogLevelFilter> for LogLevel {
    #[inline]
    fn partial_cmp(&self, other: &LogLevelFilter) -> Option<cmp::Ordering> {
        Some((*self as usize).cmp(&(*other as usize)))
    }
}

impl Ord for LogLevel {
    #[inline]
    fn cmp(&self, other: &LogLevel) -> cmp::Ordering {
        (*self as usize).cmp(&(*other as usize))
    }
}

impl LogLevel {
    fn from_usize(u: usize) -> Option<LogLevel> {
        match u {
            1 => Some(LogLevel::Error),
            2 => Some(LogLevel::Warn),
            3 => Some(LogLevel::Info),
            4 => Some(LogLevel::Debug),
            5 => Some(LogLevel::Trace),
            _ => None
        }
    }

    /// Returns the most verbose logging level.
    #[inline]
    pub fn max() -> LogLevel {
        LogLevel::Trace
    }

    /// Converts the `LogLevel` to the equivalent `LogLevelFilter`.
    #[inline]
    pub fn to_log_level_filter(&self) -> LogLevelFilter {
        LogLevelFilter::from_usize(*self as usize).unwrap()
    }
}

#[repr(usize)]
#[derive(Copy, Eq, Debug)]
pub enum LogLevelFilter {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Clone for LogLevelFilter {
    #[inline]
    fn clone(&self) -> LogLevelFilter {
        *self
    }
}

impl PartialEq for LogLevelFilter {
    #[inline]
    fn eq(&self, other: &LogLevelFilter) -> bool {
        *self as usize == *other as usize
    }
}

impl PartialEq<LogLevel> for LogLevelFilter {
    #[inline]
    fn eq(&self, other: &LogLevel) -> bool {
        other.eq(self)
    }
}

impl PartialOrd for LogLevelFilter {
    #[inline]
    fn partial_cmp(&self, other: &LogLevelFilter) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialOrd<LogLevel> for LogLevelFilter {
    #[inline]
    fn partial_cmp(&self, other: &LogLevel) -> Option<cmp::Ordering> {
        other.partial_cmp(self).map(|x| x.reverse())
    }
}

impl Ord for LogLevelFilter {
    #[inline]
    fn cmp(&self, other: &LogLevelFilter) -> cmp::Ordering {
        (*self as usize).cmp(&(*other as usize))
    }
}

impl FromStr for LogLevelFilter {
    type Err = ();
    fn from_str(level: &str) -> Result<LogLevelFilter, ()> {
        ok_or(LOG_LEVEL_NAMES.iter()
                    .position(|&name| name.eq_ignore_ascii_case(level))
                    .map(|p| LogLevelFilter::from_usize(p).unwrap()), ())
    }
}

impl LogLevelFilter {
    fn from_usize(u: usize) -> Option<LogLevelFilter> {
        match u {
            0 => Some(LogLevelFilter::Off),
            1 => Some(LogLevelFilter::Error),
            2 => Some(LogLevelFilter::Warn),
            3 => Some(LogLevelFilter::Info),
            4 => Some(LogLevelFilter::Debug),
            5 => Some(LogLevelFilter::Trace),
            _ => None
        }
    }
    /// Returns the most verbose logging level filter.
    #[inline]
    pub fn max() -> LogLevelFilter {
        LogLevelFilter::Trace
    }

    /// Converts `self` to the equivalent `LogLevel`.
    ///
    /// Returns `None` if `self` is `LogLevelFilter::Off`.
    #[inline]
    pub fn to_log_level(&self) -> Option<LogLevel> {
        LogLevel::from_usize(*self as usize)
    }
}

/// Metadata about a log message.
pub struct LogMetadata {
  pub level:  LogLevel,
  pub target: &'static str,
}

pub struct LogDirective {
    name:  Option<String>,
    level: LogLevelFilter,
}

fn ok_or<T, E>(t: Option<T>, e: E) -> Result<T, E> {
    match t {
        Some(t) => Ok(t),
        None => Err(e),
    }
}

pub fn parse_logging_spec(spec: &str) -> Vec<LogDirective> {
    let mut dirs = Vec::new();

    let mut parts = spec.split('/');
    let mods = parts.next();
    let _    = parts.next();
    if parts.next().is_some() {
        println!("warning: invalid logging spec '{}', \
                 ignoring it (too many '/'s)", spec);
        return dirs;
    }
    mods.map(|m| { for s in m.split(',') {
        if s.len() == 0 { continue }
        let mut parts = s.split('=');
        let (log_level, name) = match (parts.next(), parts.next().map(|s| s.trim()), parts.next()) {
            (Some(part0), None, None) => {
                // if the single argument is a log-level string or number,
                // treat that as a global fallback
                match part0.parse() {
                    Ok(num) => (num, None),
                    Err(_) => (LogLevelFilter::max(), Some(part0)),
                }
            }
            (Some(part0), Some(""), None) => (LogLevelFilter::max(), Some(part0)),
            (Some(part0), Some(part1), None) => {
                match part1.parse() {
                    Ok(num) => (num, Some(part0)),
                    _ => {
                        println!("warning: invalid logging spec '{}', \
                                 ignoring it", part1);
                        continue
                    }
                }
            },
            _ => {
                println!("warning: invalid logging spec '{}', \
                         ignoring it", s);
                continue
            }
        };
        dirs.push(LogDirective {
            name: name.map(|s| s.to_string()),
            level: log_level,
        });
    }});

    return dirs;
}

#[macro_export]
macro_rules! log {
    (target: $target:expr, $lvl:expr, $format:expr, $level_tag:expr, $($arg:tt)+) => ({
      use $crate::logging::LOGGER;
      static _META: $crate::logging::LogMetadata = $crate::logging::LogMetadata {
          level:  $lvl,
          target: module_path!(),
      };
      {
        //FIXME: we will lose logs in multithreading like this
        if let Ok(mut logger) = LOGGER.try_lock() {
          logger.log(
            &_META,
            format_args!(
              concat!("{}\t{}\t{}\t{}\t{}\t", $format, '\n'),
              ::time::now_utc().rfc3339(), ::time::precise_time_ns(), *$crate::logging::PID,
              $level_tag, *$crate::logging::TAG, $($arg)+));
        }
      }
    });
    (target: $target:expr, $lvl:expr, $format:expr, $level_tag:expr) => ({
      use $crate::logging::LOGGER;
      static _META: $crate::logging::LogMetadata = $crate::logging::LogMetadata {
          level:  $lvl,
          target: module_path!(),
      };
      {
        //FIXME: we will lose logs in multithreading like this
        if let Ok(mut logger) = LOGGER.try_lock() {
          logger.log(
            &_META,
            format_args!(
              concat!("{}\t{}\t{}\t{}\t{}\t", $format, '\n'),
              ::time::now_utc().rfc3339(), ::time::precise_time_ns(), *$crate::logging::PID,
              $level_tag, *$crate::logging::TAG));
        }
      }
    });
    ($lvl:expr, $($arg:tt)+) => (log!(target: module_path!(), $lvl, $($arg)+));
}

#[macro_export]
macro_rules! error {
    ($format:expr, $($arg:tt)*) => {
        log!($crate::logging::LogLevel::Error, $format, "ERROR", $($arg)*);
    };
    ($format:expr) => {
        log!($crate::logging::LogLevel::Error, $format, "ERROR");
    };
}

#[macro_export]
macro_rules! warn {
    ($format:expr, $($arg:tt)*) => {
        use time;
        use logging::PID;
        log!($crate::logging::LogLevel::Warn, $format, "WARN", $($arg)*);
    };
    ($format:expr) => {
        log!($crate::logging::LogLevel::Warn, $format, "WARN");
    }
}

#[macro_export]
macro_rules! info {
    ($format:expr, $($arg:tt)*) => {
        log!($crate::logging::LogLevel::Info, $format, "INFO", $($arg)*);
    };
    ($format:expr) => {
        log!($crate::logging::LogLevel::Info, $format, "INFO");
    }
}

#[macro_export]
macro_rules! debug {
    ($format:expr, $($arg:tt)*) => {
        #[cfg(debug_assertions)]
        log!($crate::logging::LogLevel::Debug, concat!("{}\t", $format),
          "DEBUG", module_path!(), $($arg)*);
    };
    ($format:expr) => {
        #[cfg(debug_assertions)]
        log!($crate::logging::LogLevel::Debug, concat!("{}\t", $format),
          "DEBUG", module_path!());
    }
}

#[macro_export]
macro_rules! trace {
    ($format:expr, $($arg:tt)*) => (
        #[cfg(debug_assertions)]
        log!($crate::logging::LogLevel::Trace, concat!("{}\t", $format),
          "TRACE", module_path!(), $($arg)*);
    );
    ($format:expr) => (
        #[cfg(debug_assertions)]
        log!($crate::logging::LogLevel::Trace, concat!("{}\t", $format),
          "TRACE", module_path!());
    )
}

#[macro_export]
macro_rules! setup_test_logger {
  () => (
    $crate::logging::Logger::init(module_path!().to_string(), "error", $crate::logging::LoggerBackend::Stdout(::std::io::stdout()));
  );
}
