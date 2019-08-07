use std::fs::{File, OpenOptions};
use std::sync::atomic::{self, AtomicI64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, error::Error, process, thread};

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use dirs;
use lettre::smtp::authentication::{Credentials, Mechanism};
use lettre::smtp::ConnectionReuseParameters;
use lettre::{Envelope, SendableEmail, SmtpClient, SmtpTransport, Transport};
use log::{debug, error, info, LevelFilter};
use serde::{Deserialize, Serialize};
use serde_json;
use signal_hook;
use simplelog::WriteLogger;

const BASE_LINE: Duration = Duration::from_secs(1_565_197_800);
// 07-08-2019-13:10 NY time. (17:10 UTC).
const FIVE_MINUTES: i64 = 5;
const DAY: i64 = 24 * 60; // In Minutes.

const ORDER: Ordering = Ordering::SeqCst;
const LOG_FILE: &str = ".alarm_integral.log";
const STATE_FILE: &str = ".alarm_integral.state";
const GMAIL: &str = "smtp.gmail.com";

static mut COUNTER: AtomicI64 = AtomicI64::new(0);
static mut LAST_ALARM: SystemTime = UNIX_EPOCH;

fn main() {
    demonize();
    init_logger();
    register_signal();
    if try_to_restore_state().is_err() {
        unsafe {
            LAST_ALARM += BASE_LINE;
        }
    }

    let start_message = unsafe {
        format!("Starting, last time setted is: {}, with counter: {}", DateTime::<Utc>::from(LAST_ALARM), COUNTER.load(ORDER))
    };

    debug!("{}", start_message);
    let envelop = Envelope::new(None, vec!["elichai.turkel@gmail.com".parse().unwrap()]).unwrap();

    let username = env::var("USERNAME").expect("Please Provide env var, USERNAME");
    let pass = env::var("PASS").expect("Please Provide env var, USERNAME");

    let mut transport = SmtpClient::new_simple(GMAIL)
        .unwrap()
        .credentials(Credentials::new(username, pass))
        .smtp_utf8(true)
        .authentication_mechanism(Mechanism::Plain)
        .connection_reuse(ConnectionReuseParameters::ReuseUnlimited)
        .transport();
    send_email(&envelop, "start id", start_message, &mut transport);

    let mut next = next_duration();
    let sys_next = unsafe { LAST_ALARM + next.to_std().unwrap() };
    if sys_next > SystemTime::now() {
        let sys_dur = sys_next.duration_since(SystemTime::now()).unwrap();

        next = ChronoDuration::seconds(sys_dur.as_secs() as i64);
    }
    loop {
        {
            let hours = next.num_hours();
            let mins = next.num_minutes() - (hours * 60);
            debug!("sleeping for {}:{} hours", hours, mins);
        }

        thread::sleep(next.to_std().unwrap());

        unsafe { LAST_ALARM = SystemTime::now() };

        let msg =
            unsafe { format!("Reminder for the {} time, sent time: {}", COUNTER.load(ORDER), DateTime::<Utc>::from(LAST_ALARM)) };
        send_email(&envelop, "id", msg, &mut transport);
        next = next_duration();
    }
}

fn send_email(envelop: &Envelope, id: &str, message: String, sender: &mut SmtpTransport) {
    let email = SendableEmail::new(envelop.clone(), id.to_string(), message.clone().into_bytes());
    if let Err(e) = sender.send(email) {
        error!("failed to send email, error: {:?}. content: {}", e, message);
    } else {
        info!("Successfully sent message: {}", message);
    }
}

fn next_duration() -> ChronoDuration {
    let x = unsafe { COUNTER.fetch_add(1, ORDER) } + 1;
    let next = FIVE_MINUTES * (x.pow(2)) + DAY; // f(x) = 5x^2 + c; (c=start_time, and need to add 24 hours.)
    ChronoDuration::minutes(next)
}

fn init_logger() {
    let mut log_location = dirs::home_dir().unwrap();
    log_location.push(LOG_FILE);
    WriteLogger::init(LevelFilter::Info, Default::default(), File::create(log_location).unwrap()).unwrap()
}

fn demonize() {
    let mut args = env::args();
    if args.len() == 1 {
        let child = process::Command::new(args.next().unwrap()).arg("child").spawn().unwrap();
        println!("child: {}", child.id());
        process::exit(0);
    } else {
        println!("Daemonized!, id: {}", process::id());
    }
}

fn register_signal() {
    unsafe {
        signal_hook::register(signal_hook::SIGTERM, save_status_to_file_and_exit).expect("Failed to set signal");
        signal_hook::register(signal_hook::SIGHUP, save_status_to_file_and_exit).expect("Failed to set signal");
        signal_hook::register(signal_hook::SIGQUIT, save_status_to_file_and_exit).expect("Failed to set signal");
        signal_hook::register(signal_hook::SIGINT, save_status_to_file_and_exit).expect("Failed to set signal");
        signal_hook::register(signal_hook::SIGABRT, save_status_to_file_and_exit).expect("Failed to set signal");
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct State {
    pub counter: i64,
    pub time: SystemTime,
}

fn save_status_to_file_and_exit() {
    let mut save_location = dirs::home_dir().expect("Failed to save state");
    save_location.push(STATE_FILE);
    let file = OpenOptions::new().write(true).create(true).open(save_location).expect("Failed to save state");
    atomic::compiler_fence(ORDER);
    let state = unsafe { State { counter: COUNTER.load(ORDER), time: LAST_ALARM } };
    serde_json::to_writer(file, &state).expect("Failed to save state");
    process::exit(1);
}

fn try_to_restore_state() -> Result<(), Box<dyn Error>> {
    let mut save_location = dirs::home_dir().unwrap();
    save_location.push(STATE_FILE);
    let f = File::open(save_location)?;
    let state: State = serde_json::from_reader(f)?;
    unsafe { COUNTER.swap(state.counter, ORDER) };
    unsafe { LAST_ALARM = state.time };
    // TODO: Fast forward the calculation.
    Ok(())
}
