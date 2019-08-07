use chrono::{DateTime, Duration as ChronoDuration, Utc};
use dirs;
use lettre::{sendmail::SendmailTransport, Envelope, SendableEmail, Transport};
use log::{debug, error, info, LevelFilter};
use simplelog::WriteLogger;
use std::env;
use std::fs::File;
use std::process;
use std::thread;
use std::time::{Duration, UNIX_EPOCH};

const BASE_LINE: Duration = Duration::from_secs(1_565_197_800); // 07-08-2019-13:10 NY time. (17:10 UTC).
const FIVE_MINUTES: i64 = 5;
const DAY: i64 = 24 * 60; // In Minutes.

fn main() {
    demonize();
    init_logger();

    let mut last_alarm = DateTime::from(UNIX_EPOCH + BASE_LINE);
    let envelop = Envelope::new(None, vec!["elichai.turkel@gmail.com".parse().unwrap()]).unwrap();
    let mut transport = SendmailTransport::new();

    let mut counter = 0;
    loop {
        let next = next_duration(&mut counter);
        {
            let hours = next.num_hours();
            let mins = next.num_minutes() - (hours * 60);
            debug!("sleeping for {}:{} hours", hours, mins);
        }

        thread::sleep(next.to_std().unwrap());

        last_alarm = Utc::now();
        let msg = format!("Reminder for the {} time, sent time: {}", counter, last_alarm);
        send_email(&envelop, "id", msg, &mut transport);
    }
}

fn send_email(envelop: &Envelope, id: &str, message: String, sender: &mut SendmailTransport) {
    let email = SendableEmail::new(envelop.clone(), id.to_string(), message.clone().into_bytes());
    if let Err(e) = sender.send(email) {
        error!("failed to send email, error: {:?}. content: {}", e, message);
    } else {
        info!("Successfully sent message: {}", message);
    }
}

fn next_duration(x: &mut i64) -> ChronoDuration {
    *x += 1;
    let next = FIVE_MINUTES * (x.pow(2)) + DAY; // f(x) = 5x^2 + c; (c=start_time, and need to add 24 hours.)
    ChronoDuration::minutes(next)
}

fn init_logger() {
    let mut log_location = dirs::home_dir().unwrap();
    log_location.push(".alarm_integral.log");
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
