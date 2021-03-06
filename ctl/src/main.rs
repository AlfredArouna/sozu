#[macro_use] extern crate prettytable;
extern crate rand;
extern crate sozu_command_lib as sozu_command;
extern crate structopt;
#[macro_use] extern crate structopt_derive;

mod command;
mod cli;

use structopt::StructOpt;

use sozu_command::config::Config;
use sozu_command::channel::Channel;
use sozu_command::data::{ConfigMessage,ConfigMessageAnswer};

use command::{add_application,remove_application,dump_state,load_state,
  save_state,soft_stop,hard_stop,upgrade,status,metrics,
  remove_backend, add_backend, remove_http_frontend, add_http_frontend,
  remove_tcp_frontend, add_tcp_frontend, add_certificate, remove_certificate,
  query_application, logging_filter};

use cli::*;

fn main() {
  let matches = App::from_args();

  let config_file = matches.config;

  let config = Config::load_from_path(config_file.as_str()).expect("could not parse configuration file");
  let mut channel: Channel<ConfigMessage,ConfigMessageAnswer> =
    Channel::from_path(&config.command_socket_path(), 10_000, 2_000_000)
      .expect("could not connect to the command unix socket");

  channel.set_nonblocking(false);

  match matches.cmd {
    SubCmd::Shutdown{ hard } => {
      if hard {
        hard_stop(&mut channel);
      } else {
        soft_stop(&mut channel);
      }
    },
    SubCmd::Upgrade => upgrade(&mut channel),
    SubCmd::Status => status(channel),
    SubCmd::Metrics => metrics(&mut channel),
    SubCmd::Logging{ level } => logging_filter(&mut channel, &level),
    SubCmd::State{ cmd } => {
      match cmd {
        StateCmd::Save{ file } => save_state(&mut channel, &file),
        StateCmd::Load{ file } => load_state(channel, file),
        StateCmd::Dump => dump_state(&mut channel),
      }
    },
    SubCmd::Application{ cmd } => {
      match cmd {
        ApplicationCmd::Add{ id, sticky_session } => add_application(&mut channel, &id, sticky_session),
        ApplicationCmd::Remove{ id } => remove_application(&mut channel, &id),
      }
    },
    SubCmd::Backend{ cmd } => {
      match cmd {
        BackendCmd::Add{ id, instance_id, ip, port } => add_backend(&mut channel, &id, &instance_id, &ip, port),
        BackendCmd::Remove{ id, instance_id, ip, port } => remove_backend(&mut channel, &id, &instance_id, &ip, port),
      }
    },
    SubCmd::Frontend{ cmd } => {
      match cmd {
        FrontendCmd::Http{ cmd } => match cmd {
          HttpFrontendCmd::Add{ id, hostname, path_begin, path_to_certificate } =>
            add_http_frontend(&mut channel, &id, &hostname, &path_begin.unwrap_or("".to_string()), path_to_certificate),
          HttpFrontendCmd::Remove{ id, hostname, path_begin, path_to_certificate } =>
            remove_http_frontend(&mut channel, &id, &hostname, &path_begin.unwrap_or("".to_string()), path_to_certificate),
        },
        FrontendCmd::Tcp { cmd } => match cmd {
          TcpFrontendCmd::Add{ id, ip_address, port } =>
            add_tcp_frontend(&mut channel, &id, &ip_address, port),
          TcpFrontendCmd::Remove{ id, ip_address, port } =>
            remove_tcp_frontend(&mut channel, &id, &ip_address, port),
        }
      }
    },
    SubCmd::Certificate{ cmd } => {
      match cmd {
        CertificateCmd::Add{ certificate, chain, key } => add_certificate(&mut channel, &certificate, &chain, key.unwrap_or("missing key path".to_string()).as_str()),
        CertificateCmd::Remove{ certificate } => remove_certificate(&mut channel, &certificate),
      }
    },
    SubCmd::Query{ cmd } => {
      match cmd {
        QueryCmd::Applications{ id } => query_application(&mut channel, id),
      }
    },
  }
}
