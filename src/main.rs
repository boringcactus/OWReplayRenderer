#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate lazy_static;

mod image;
mod obs;
mod window;

use crate::image::{InReplay, Screenshot};
use obs::*;
use std::env::{current_exe, set_current_dir};
use std::ffi::OsString;
use std::fs::read_dir;
use std::io::{stdin, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;
use window::*;

lazy_static! {
    static ref RUNNING: Arc<AtomicBool> = { Arc::new(AtomicBool::new(true)) };
}

fn main() {
    println!(
        r#"Thanks for using OWReplayRenderer, brought to you by boringcactus.
Before we get started, make sure everything's all ready to go:
- OBS and Overwatch are both running
- OBS has `obs-websocket` installed and running on port 4444 with no authentication
- Overwatch has all the default keybinds for the replay viewer: F1-F12 for player focus, Ctrl+P for pause, N to show/hide controls
- Overwatch has Ctrl+Left bound to 'Jump to Start' and Ctrl+Right bound to 'Replay Forward'
- Load up a replay, spectate yourself with one of F1-F12, take a 1080p screenshot of the whole screen, and save it next to OWReplayRenderer.exe as "username_badge.png"
Got all that? Press Enter to continue."#
    );
    let _ = read_line();

    if !Screenshot::<InReplay>::has_me() {
        // if we didn't find it in the existing working directory, find it adjacent to the executable
        if let Ok(x) = current_exe() {
            if let Some(x) = x.parent() {
                match set_current_dir(x) {
                    Ok(_) => (),
                    Err(x) => eprintln!("Error looking for screenshot: {}", x),
                }
            }
        }
    }

    while !Screenshot::<InReplay>::has_me() {
        println!(
            r#"Couldn't find a screenshot with your username.
Load up a replay, spectate yourself with one of F1-F12, take a 1080p screenshot of the whole screen, and save it next to OWReplayRenderer.exe as "username_badge.png".
Press Enter when you've done that."#
        );
        let _ = read_line();
    }

    let replays = read_replay_range();

    println!("Go make sure Overwatch is at the main menu, then come back here and press Enter.");
    let _ = read_line();

    println!(
        r"That's all we need! You'll need to re-focus Overwatch yourself, so this tool can send it keyboard shortcuts.
It'll render each entire game from the perspective of each player on your team, which will take a while.
It'll record the oldest replay first and work its way forward.
You can't do anything else with your computer during that time, either, unfortunately.
Once everything is rendered, it'll exit the replay viewer automatically, and stitch those videos together for easier viewing.
Alt-tab back into Overwatch and then come back in a long time."
    );

    {
        let r = RUNNING.clone();

        ctrlc::set_handler(move || {
            r.store(false, Ordering::SeqCst);
        })
        .expect("Error setting Ctrl-C handler");
    }

    let replay_count = replays.len();
    for (i, index) in replays.into_iter().rev().enumerate() {
        let mut obs = OBSClient::new();
        let record_dir = obs.use_subdir();

        record(&mut obs, index, &record_dir);
        if !RUNNING.load(Ordering::SeqCst) {
            return;
        }
        mux(record_dir);
        if !RUNNING.load(Ordering::SeqCst) {
            return;
        }

        println!("Finished recording game {}/{}", i + 1, replay_count);
    }

    println!("Done with everything! Press Enter to exit.");
    let _ = read_line();
}

fn read_replay_range() -> Vec<u8> {
    println!(
        r#"This tool can record whichever replays you want. Enter a range or set of ranges (e.g. "1-4, 6-7, 9"):"#
    );
    let line = read_line();
    let pieces = line.split(',').map(|x| x.trim());
    let mut result = vec![];
    for piece in pieces {
        let range: Vec<&str> = piece.splitn(2, "-").collect();
        let bounds = match range.as_slice() {
            [n] => n.parse::<u8>().map(|x| (x, x)),
            [a, b] => a
                .parse::<u8>()
                .and_then(|a| b.parse::<u8>().map(|b| (a, b))),
            _ => unreachable!(),
        };
        let (lo, hi) = match bounds {
            Ok((lo, hi)) => {
                if lo > hi {
                    println!("Bad range: {}-{} is not valid", lo, hi);
                    return read_replay_range();
                }
                if lo == 0 || lo > 10 {
                    println!("Bad range: {} is not valid", lo);
                    return read_replay_range();
                }
                if hi > 10 {
                    println!("Bad range: {} is not valid", hi);
                    return read_replay_range();
                }
                (lo, hi)
            }
            Err(e) => {
                println!("Bad range: {}", e);
                return read_replay_range();
            }
        };
        result.extend(lo..=hi);
    }
    result.sort();
    result
}

#[derive(Copy, Clone)]
enum Side {
    Red,
    Blue,
}

impl Into<Vec<Key>> for Side {
    fn into(self) -> Vec<Key> {
        match self {
            Side::Blue => return vec![F1, F2, F3, F4, F5, F6],
            Side::Red => return vec![F7, F8, F9, F10, F11, F12],
        }
    }
}

fn guess_side(obs: &mut OBSClient, overwatch: &Window) -> Side {
    // skip forward a bit
    big_sleep();
    overwatch.send(&ctrl(Right));
    big_sleep();
    overwatch.send(&ctrl(Right));
    big_sleep();
    overwatch.send(&ctrl(Right));
    big_sleep();

    // see if we can find the player
    let best = [Side::Blue, Side::Red].iter().map(|&side| {
        let keys: Vec<Key> = side.into();
        let max = keys.iter().map(|key| {
            overwatch.send(key);
            big_sleep();
            obs.get_screenshot::<InReplay>().is_me_score()
        }).max_by(|n1, n2| n1.partial_cmp(n2).expect("Couldn't compare floats")).expect("Couldn't compare floats");
        (side, max)
    }).max_by(|(_, n1), (_, n2)| n1.partial_cmp(n2).expect("Couldn't compare floats"));
    best.expect("Couldn't compare floats").0
}

fn record(obs: &mut OBSClient, index: u8, record_dir: &PathBuf) {
    let overwatch = Window::overwatch();

    overwatch.await_focus();

    // open the replays tab
    small_sleep();
    overwatch.send(&Up);
    overwatch.send(&Up);
    overwatch.send(&Up);
    overwatch.send(&Up);
    overwatch.send(&Space);
    big_sleep();
    overwatch.click(380, 62);
    big_sleep();
    if !RUNNING.load(Ordering::SeqCst) {
        return;
    }

    // open the replay
    for _ in 0..index {
        overwatch.send(&Down);
    }
    overwatch.send(&Tab);
    overwatch.send(&Space);
    if !RUNNING.load(Ordering::SeqCst) {
        return;
    }

    // wait for it to load
    sleep(Duration::from_secs(10));
    if !RUNNING.load(Ordering::SeqCst) {
        return;
    }

    // pause it
    overwatch.send(&ctrl(P));

    // guess the side
    let side = guess_side(obs, &overwatch);
    let side: Vec<Key> = side.into();
    if !RUNNING.load(Ordering::SeqCst) {
        return;
    }

    for player in side {
        record_once(player, obs, &overwatch, record_dir);
        if !RUNNING.load(Ordering::SeqCst) {
            return;
        }
    }

    // quit from this replay (click to dismiss the controls if they are shown)
    big_sleep();
    overwatch.click(1710, 1003);
    overwatch.send(&Escape);
    overwatch.send(&Up);
    overwatch.send(&Up);
    overwatch.send(&Space);
    big_sleep();

    println!("Finished recording everyone's perspective!")
}

fn read_line() -> String {
    let stdin = stdin();
    let mut result = String::new();
    stdin.read_line(&mut result).expect("Couldn't read from stdin");
    result.trim().to_string()
}

fn record_once(player: Key, obs: &mut OBSClient, overwatch: &Window, record_dir: &PathBuf) {
    // make sure we don't start while overwatch is not focused
    overwatch.await_focus();
    // tell overwatch to watch the designated player
    overwatch.send(&player);
    // tell overwatch to skip to the beginning
    overwatch.send(&ctrl(Left));
    // give it a while to re-load
    big_sleep();
    big_sleep();
    if !RUNNING.load(Ordering::SeqCst) {
        return;
    }
    // dismiss the controls if they're shown
    overwatch.click(1710, 1003);
    big_sleep();
    // show the controls
    overwatch.send(&N);
    big_sleep();
    // if it's not definitely paused...
    if !obs.get_screenshot::<InReplay>().is_definitely_paused() {
        // pause it
        overwatch.send(&ctrl(P));
        // skip to the beginning again
        overwatch.send(&ctrl(Left));
    }
    // dismiss the controls
    overwatch.send(&N);
    // chase the target
    overwatch.send(&player);
    // tell OBS to start recording
    obs.start_recording();
    // wait a bit so OBS can catch up
    big_sleep();
    // tell overwatch to unpause
    overwatch.send(&ctrl(P));
    // while the game hasn't ended...
    while !obs.get_screenshot::<InReplay>().is_gameover() {
        // spam
        overwatch.send(&player);
        med_sleep();
        if !RUNNING.load(Ordering::SeqCst) {
            return;
        }
    }
    // wait another while
    big_sleep();
    // stop recording
    obs.stop_recording();
    // wait a bit
    big_sleep();
    // jump to beginning again
    overwatch.send(&ctrl(Left));
    big_sleep();
    // re-pause since reaching end doesn't actually pause
    overwatch.send(&ctrl(P));
    rename(record_dir);
    print!("{:?} done. ", player);
    std::io::stdout().flush().expect("Couldn't flush stdout");
}

pub fn small_sleep() {
    sleep(Duration::from_millis(200));
}

pub fn med_sleep() {
    sleep(Duration::from_secs(1));
}

pub fn big_sleep() {
    sleep(Duration::from_secs(2));
}

const MUX_ALL: bool = false;

/// Multiplex all those pieces into a video file with one track for each video,
/// plus one track with a whole matrix overview exclusively for the purpose of flexing.
fn mux(record_dir: PathBuf) {
    if !has_ffmpeg() {
        return;
    }
    let cameras = read_dir(&record_dir)
        .expect("Couldn't read record dir for muxing")
        .filter_map(|x| x.ok())
        .map(|x| x.file_name())
        .filter(|x| x != "final.mkv" && x != "mosaic.mkv")
        .collect::<Vec<_>>();
    let mut inputs = cameras
        .into_iter()
        .flat_map(|x| vec![OsString::from("-i"), x])
        .collect::<Vec<_>>();

    println!("Building mosaic...");
    let filter = r"
        nullsrc=size=1920x720:r=60 [base];
        [0:v] setpts=PTS-STARTPTS, scale=640x360 [upperleft];
        [1:v] setpts=PTS-STARTPTS, scale=640x360 [uppermiddle];
        [2:v] setpts=PTS-STARTPTS, scale=640x360 [upperright];
        [3:v] setpts=PTS-STARTPTS, scale=640x360 [lowerleft];
        [4:v] setpts=PTS-STARTPTS, scale=640x360 [lowermiddle];
        [5:v] setpts=PTS-STARTPTS, scale=640x360 [lowerright];
        [base][upperleft] overlay=shortest=1 [tmp1];
        [tmp1][uppermiddle] overlay=shortest=1:x=640 [tmp2];
        [tmp2][upperright] overlay=shortest=1:x=1280 [tmp3];
        [tmp3][lowerleft] overlay=shortest=1:y=360 [tmp4];
        [tmp4][lowermiddle] overlay=shortest=1:y=360:x=640 [tmp5];
        [tmp5][lowerright] overlay=shortest=1:y=360:x=1280
    ";
    let result = Command::new("ffmpeg")
        .args(&["-y", "-hide_banner", "-v", "warning", "-nostats"])
        .args(&inputs)
        .arg("-filter_complex")
        .arg(filter)
        .args(&[
            "-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-an",
        ])
        .arg("mosaic.mkv")
        .current_dir(&record_dir)
        .status()
        .expect("Couldn't build mosaic");
    if !result.success() {
        panic!(
            "ffmpeg failed with code {}",
            result.code().map_or("?".to_string(), |x| x.to_string())
        )
    }
    if !RUNNING.load(Ordering::SeqCst) {
        return;
    }

    if !MUX_ALL {
        let mut src = record_dir.clone();
        src.push("mosaic.mkv");
        let mut dest = PathBuf::from(record_dir.parent().expect("No path parent").clone());
        dest.push(format!(
            "done_mosaic_{}.mkv",
            record_dir
                .file_name()
                .and_then(|x| x.to_str())
                .expect("Failed to get directory name")
        ));
        std::fs::rename(src, dest).expect("Failed to move mosaic");
        return;
    }

    inputs.append(&mut vec![
        OsString::from("-i"),
        OsString::from("mosaic.mkv"),
    ]);

    println!("Merging...");
    let out_name = {
        let mut path = PathBuf::from("..");
        path.push(
            record_dir
                .file_name()
                .expect("Failed to get directory name"),
        );
        path.with_extension("mkv")
    };
    let maps = (0..(inputs.len() / 2))
        .flat_map(|x| vec!["-map".to_string(), format!("{}", x)])
        .collect::<Vec<_>>();
    let result = Command::new("ffmpeg")
        .args(&["-y", "-hide_banner", "-v", "warning", "-stats"])
        .args(inputs)
        .args(&[
            "-filter_complex",
            "[0:a][1:a][2:a][3:a][4:a][5:a] amix=inputs=6",
        ])
        .args(maps)
        .args(&["-c:v", "copy", "-c:a", "aac"])
        .arg(&out_name)
        .current_dir(&record_dir)
        .status()
        .expect("Couldn't mux");
    if !result.success() {
        panic!(
            "ffmpeg failed with code {}",
            result.code().map_or("?".to_string(), |x| x.to_string())
        )
    }
    if !RUNNING.load(Ordering::SeqCst) {
        return;
    }
}

fn has_ffmpeg() -> bool {
    let result = Command::new("ffmpeg")
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("Couldn't try to detect ffmpeg");
    result.success()
}

pub fn rename(record_dir: &PathBuf) {
    let not_done = read_dir(record_dir)
        .expect("Couldn't read record dir for renaming")
        .filter_map(|x| x.ok())
        .filter_map(|x| x.file_name().into_string().ok())
        .filter(|x| !x.starts_with("done_"));
    for file in not_done {
        let src = record_dir.join(&file);
        let dest = record_dir.join(format!("done_{}", &file));
        ::std::fs::rename(src, dest).expect("Couldn't rename");
    }
}

pub fn timestamp() -> String {
    use chrono::prelude::*;
    let now: DateTime<Local> = Local::now();
    now.format("%Y-%m-%d %H-%M-%S").to_string()
}
