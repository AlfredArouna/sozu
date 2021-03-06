use mio_uds::UnixStream;
use mio::Token;
use libc::{self,pid_t};
use std::process::Command;
use std::os::unix::process::CommandExt;
use std::os::unix::io::{AsRawFd,FromRawFd};
use std::fs::File;
use std::io::{Seek,SeekFrom};
use nix::unistd::*;
use serde_json;
use tempfile::tempfile;

use sozu_command::config::Config;
use sozu_command::data::RunState;
use sozu_command::channel::Channel;
use sozu_command::state::ConfigState;
use sozu_command::messages::OrderMessage;

use util;
use logging;
use command::{CommandServer,Worker};
use worker::get_executable_path;

#[derive(Deserialize,Serialize,Debug)]
pub struct SerializedWorker {
  pub fd:         i32,
  pub pid:        i32,
  pub id:         u32,
  pub run_state:  RunState,
  pub token:      Option<usize>,
  pub queue:      Vec<OrderMessage>,
}

impl SerializedWorker {
  pub fn from_proxy(proxy: &Worker) -> SerializedWorker {
    SerializedWorker {
      fd:         proxy.channel.sock.as_raw_fd(),
      pid:        proxy.pid,
      id:         proxy.id,
      run_state:  proxy.run_state.clone(),
      token:      proxy.token.clone().map(|Token(t)| t),
      queue:      proxy.queue.clone().into(),
    }
  }
}

#[derive(Deserialize,Serialize,Debug)]
pub struct UpgradeData {
  pub command:     i32,
  //clients: ????
  pub config:      Config,
  pub workers:     Vec<SerializedWorker>,
  pub state:       ConfigState,
  pub next_id:     u32,
  pub token_count: usize,
  //pub order_state: HashMap<String, HashSet<usize>>,
}

pub fn start_new_master_process(upgrade_data: UpgradeData) -> (pid_t, Channel<(),bool>) {
  trace!("parent({})", unsafe { libc::getpid() });

  let mut upgrade_file = tempfile().expect("could not create temporary file for upgrade");

  util::disable_close_on_exec(upgrade_file.as_raw_fd());

  serde_json::to_writer(&mut upgrade_file, &upgrade_data).expect("could not write upgrade data to temporary file");
  upgrade_file.seek(SeekFrom::Start(0)).expect("could not seek to beginning of file");

  let (server, client) = UnixStream::pair().unwrap();

  util::disable_close_on_exec(client.as_raw_fd());

  let mut command: Channel<(),bool> = Channel::new(
    server,
    upgrade_data.config.command_buffer_size,
    upgrade_data.config.max_command_buffer_size
  );
  command.set_nonblocking(false);

  let path = unsafe { get_executable_path() };

  info!("launching new master");
  //FIXME: remove the expect, return a result?
  match fork().expect("fork failed") {
    ForkResult::Parent{ child } => {
      info!("master launched: {}", child);
      command.set_nonblocking(true);

      return (child, command);
    }
    ForkResult::Child => {
      trace!("child({}):\twill spawn a child", unsafe { libc::getpid() });
      let res = Command::new(path)
        .arg("upgrade")
        .arg("--fd")
        .arg(client.as_raw_fd().to_string())
        .arg("--upgrade-fd")
        .arg(upgrade_file.as_raw_fd().to_string())
        .arg("--channel-buffer-size")
        .arg(upgrade_data.config.command_buffer_size.to_string())
        .exec();

      error!("exec call failed: {:?}", res);
      unreachable!();
    }
  }
}

pub fn begin_new_master_process(fd: i32, upgrade_fd: i32, channel_buffer_size: usize) {
  let mut command: Channel<bool,()> = Channel::new(
    unsafe { UnixStream::from_raw_fd(fd) },
    channel_buffer_size,
    channel_buffer_size *2
  );

  command.set_blocking(true);

  let upgrade_file = unsafe { File::from_raw_fd(upgrade_fd) };
  let upgrade_data: UpgradeData = serde_json::from_reader(upgrade_file).expect("could not parse upgrade data");

  //FIXME: should have an id for the master too
  logging::setup("MASTER".to_string(), &upgrade_data.config.log_level, &upgrade_data.config.log_target);
  //info!("new master got upgrade data: {:?}", upgrade_data);

  let mut server = CommandServer::from_upgrade_data(upgrade_data);
  server.enable_cloexec_after_upgrade();
  info!("starting new master loop");
  command.write_message(&true);
  server.run();
  info!("master process stopped");

}
