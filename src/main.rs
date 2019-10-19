#[macro_use]
extern crate serde_json;

mod obs;
mod window;

use obs::*;
use std::io::stdin;
use std::thread::sleep;
use std::time::Duration;
use window::*;

fn main() {
    println!(r"Thanks for using OWReplayRenderer, brought to you by boringcactus.
Before we get started, make sure everything's all ready to go:
- OBS and Overwatch are both running
- OBS has `obs-websocket` installed and running on port 4444 with no authentication
- Overwatch has all the default keybinds for the replay viewer: F1-F12 for player focus, Ctrl+P for pause
- Overwatch has Ctrl+Left bound to 'Jump to Start'
Got all that? Press Enter to continue.");
    let _ = read_line();
    println!(
        r"Sweet, now get your specific replay ready.
- Open the replay, let it load, and pause it
- Check the max timecode the interface shows on the scrub bar (ex. 00:09:31)"
    );
    let replay_length = get_replay_length();
    println!("Which team do you want to see? Red or blue?");
    let side = get_side();
    println!(r"That's all we need! You'll need to re-focus Overwatch yourself, so this tool can send it keyboard shortcuts.
It'll render the entire game from each player's perspective, which will take a while.
You can't do anything else with your computer during that time, either, unfortunately.
Once everything is rendered, it'll exit the replay viewer automatically; when that happens, alt+tab back here and
this tool will help you stitch those videos all together for easier viewing.");
    println!(
        "Alt-tab back into Overwatch and then come back in *checks notes* {} minutes",
        6 * replay_length.as_secs() / 60
    );

    let mut obs = OBSClient::new();
    let overwatch = Window::overwatch();

    overwatch.await_focus();

    for player in side {
        record_once(player, &replay_length, &mut obs, &overwatch);
    }

    small_sleep();
    overwatch.send(&Escape);
    small_sleep();
    overwatch.send(&Up);
    small_sleep();
    overwatch.send(&Up);
    small_sleep();
    overwatch.send(&Space);
    small_sleep();
}

fn read_line() -> String {
    let stdin = stdin();
    let mut result = String::new();
    stdin.read_line(&mut result).unwrap();
    result.trim().to_string()
}

fn get_replay_length() -> Duration {
    loop {
        println!("Enter the replay length (ex. 00:09:31):");
        let text = read_line();
        let pieces: Vec<&str> = text.split(":").collect();
        let int_pieces: Vec<Result<u64, _>> = pieces.iter().map(|x| x.parse()).collect();
        match int_pieces.as_slice() {
            [Ok(h), Ok(m), Ok(s)] => {
                let m = h * 60 + m;
                let s = m * 60 + s;
                return Duration::from_secs(s);
            },
            _ => println!("Invalid replay length. Enter hours, followed by a colon, followed by minutes, followed by a colon, followed by seconds.")
        }
    }
}

fn get_side() -> Vec<Key> {
    loop {
        println!("Enter \"red\" or \"blue\":");
        let text = read_line();
        match text.to_lowercase().as_str() {
            "blue" => return vec![F1, F2, F3, F4, F5, F6],
            "red" => return vec![F7, F8, F9, F10, F11, F12],
            _ => println!("That's not \"red\" or \"blue\"! Which side do you want to see?"),
        }
    }
}

fn record_once(player: Key, replay_length: &Duration, obs: &mut OBSClient, overwatch: &Window) {
    // tell overwatch to watch the designated player
    overwatch.send(&player);
    small_sleep();
    // tell overwatch to skip to the beginning
    overwatch.send(&ctrl(Left));
    // tell OBS to start recording
    obs.start_recording();
    // wait a bit so OBS can catch up
    big_sleep();
    // tell overwatch to unpause
    overwatch.send(&ctrl(P));
    // wait for the game to end
    sleep(replay_length.clone());
    // wait another second
    med_sleep();
    // stop recording
    obs.stop_recording();
    // wait a bit
    sleep(Duration::from_secs(2));
    // jump to beginning again
    overwatch.send(&ctrl(Left));
    small_sleep();
    // re-pause since reaching end doesn't actually pause
    overwatch.send(&ctrl(P));
    println!("Recording following user {:?} finished", player);
}

fn small_sleep() {
    sleep(Duration::from_millis(100));
}

fn med_sleep() {
    sleep(Duration::from_secs(1));
}

fn big_sleep() {
    sleep(Duration::from_secs(2));
}
