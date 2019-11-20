#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate lazy_static;

mod image;
mod obs;
mod ocr;
mod window;

use obs::*;
use std::ffi::OsString;
use std::fs::read_dir;
use std::io::stdin;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::thread::sleep;
use std::time::{Duration, Instant};
use window::*;
use crate::image::ReplaysMenu;

fn test() {
    let mut obs = OBSClient::new();
    let screenshot = obs.get_screenshot::<ReplaysMenu>();
    let replays = screenshot.get_replays();
    dbg!(replays);
}

fn main() {
    test();
    return;
    println!(
        r"Thanks for using OWReplayRenderer, brought to you by boringcactus.
Before we get started, make sure everything's all ready to go:
- OBS and Overwatch are both running
- OBS has `obs-websocket` installed and running on port 4444 with no authentication
- Overwatch has all the default keybinds for the replay viewer: F1-F12 for player focus, Ctrl+P for pause
- Overwatch has Ctrl+Left bound to 'Jump to Start'
Got all that? Press Enter to continue."
    );
    let _ = read_line();

    println!(
        r#"For each replay you want to record, enter its position in the replay list (1-10),
then its duration (minutes:seconds), then which side you want to record from that replay (red or blue).
For example:
1 12:06 red
3 15:02 blue
2 9:35 red

Leave a blank line when you're done. Make sure you're looking at the main menu when you finish. Start typing here:"#
    );
    let mut specs = vec![];
    loop {
        let line = read_line();
        if line.is_empty() {
            break;
        }
        let spec: ReplaySpec = match line.parse() {
            Ok(spec) => spec,
            Err(e) => {
                eprintln!("{}", e);
                continue;
            }
        };
        specs.push(spec);
    }

    println!(
        r"That's all we need! You'll need to re-focus Overwatch yourself, so this tool can send it keyboard shortcuts.
It'll render each entire game from each player's perspective, which will take a while.
You can't do anything else with your computer during that time, either, unfortunately.
Once everything is rendered, it'll exit the replay viewer automatically, and stitch those videos together for easier viewing."
    );
    println!(
        "Alt-tab back into Overwatch and then come back in at least *checks notes* {} minutes",
        6u64 * specs.iter().map(|x| x.1.as_secs()).sum::<u64>() / 60
    );

    let spec_count = specs.len();

    for (i, spec) in specs.into_iter().enumerate() {
        let mut obs = OBSClient::new();
        let record_dir = obs.use_subdir();

        record(&mut obs, spec);
        mux(record_dir);

        println!("Finished recording game {}/{}", i + 1, spec_count);
    }

    println!("Done with everything! Press Enter to exit.");
    let _ = read_line();
}

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

struct ReplaySpec(u8, Duration, Side);

impl FromStr for ReplaySpec {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let pieces: Vec<&str> = s.split(' ').collect();
        let [index, duration, side] = match pieces.as_slice() {
            [index, duration, side] => [index, duration, side],
            _ => return Err("Lines should be three pieces, separated by a space.".into()),
        };
        let index = match index.parse::<u8>() {
            Ok(x) => {
                if x >= 1 && x <= 10 {
                    x
                } else {
                    return Err("Index must be between 1 and 10.".into());
                }
            }
            Err(e) => return Err(e.to_string()),
        };
        let duration = {
            let pieces: Vec<&str> = duration.split(":").collect();
            let int_pieces: Vec<Result<u64, _>> = pieces.iter().map(|x| x.parse()).collect();
            match int_pieces.as_slice() {
                [Ok(m), Ok(s)] => {
                    let s = m * 60 + s;
                    Duration::from_secs(s)
                }
                _ => return Err("Replay length must be minutes:seconds.".into()),
            }
        };
        let side = match side.to_lowercase().as_str() {
            "blue" => Side::Blue,
            "red" => Side::Red,
            _ => return Err("Side must be \"red\" or \"blue\"".into()),
        };
        Ok(ReplaySpec(index, duration, side))
    }
}

fn record(obs: &mut OBSClient, spec: ReplaySpec) {
    let ReplaySpec(index, replay_length, side) = spec;
    let side: Vec<Key> = side.into();
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

    // open the replay
    for _ in 0..index {
        overwatch.send(&Down);
    }
    overwatch.send(&Tab);
    overwatch.send(&Space);

    // wait for it to load
    sleep(Duration::from_secs(10));

    // pause it
    overwatch.send(&ctrl(P));

    for player in side {
        record_once(player, &replay_length, obs, &overwatch);
    }

    // quit from this replay
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
    stdin.read_line(&mut result).unwrap();
    result.trim().to_string()
}

fn record_once(player: Key, replay_length: &Duration, obs: &mut OBSClient, overwatch: &Window) {
    // make sure we don't start while overwatch is not focused
    overwatch.await_focus();
    // tell overwatch to watch the designated player
    overwatch.send(&player);
    // tell overwatch to skip to the beginning
    overwatch.send(&ctrl(Left));
    // tell OBS to start recording
    obs.start_recording();
    // wait a bit so OBS can catch up
    big_sleep();
    // tell overwatch to unpause
    overwatch.send(&ctrl(P));
    // can't just wait for the game to end bc if ppl aren't loaded in at the start
    // then we don't have them available to focus. instead, we spam once a second until the game ends
    let game_end = Instant::now() + replay_length.clone();
    while Instant::now() < game_end {
        overwatch.send(&player);
        med_sleep();
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
    print!("{:?} done. ", player);
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
        .unwrap()
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
        .unwrap();
    if !result.success() {
        panic!(
            "ffmpeg failed with code {}",
            result.code().map_or("?".to_string(), |x| x.to_string())
        )
    }

    if !MUX_ALL {
        let mut src = record_dir.clone();
        src.push("mosaic.mkv");
        let mut dest = PathBuf::from(record_dir.parent().expect("No path parent").clone());
        dest.push(format!(
            "mosaic_{}.mkv",
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
        .unwrap();
    if !result.success() {
        panic!(
            "ffmpeg failed with code {}",
            result.code().map_or("?".to_string(), |x| x.to_string())
        )
    }
}

fn has_ffmpeg() -> bool {
    let result = Command::new("ffmpeg")
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    result.success()
}

pub fn timestamp() -> String {
    use chrono::prelude::*;
    let now: DateTime<Local> = Local::now();
    now.format("%Y-%m-%d %H-%M-%S").to_string()
}
