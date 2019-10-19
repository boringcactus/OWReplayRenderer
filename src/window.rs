use winapi::shared::minwindef::{BOOL, FALSE, LPARAM, TRUE};
use winapi::shared::ntdef::LPSTR;
use winapi::shared::windef::HWND;
use winapi::um::winuser::{
    EnumWindows, GetForegroundWindow, GetWindowTextA, GetWindowTextLengthA, SendInput, INPUT,
    INPUT_KEYBOARD, KEYEVENTF_KEYUP, VK_ESCAPE, VK_F1, VK_F10, VK_F11, VK_F12, VK_F2, VK_F3, VK_F4,
    VK_F5, VK_F6, VK_F7, VK_F8, VK_F9, VK_LCONTROL, VK_LEFT, VK_SPACE, VK_UP,
};

#[derive(Debug, Copy, Clone)]
enum Target {
    Overwatch,
}

impl Target {
    fn pattern(&self) -> &'static str {
        match self {
            Target::Overwatch => "Overwatch",
        }
    }
}

struct WindowSearch {
    handle: Option<HWND>,
    target: Target,
}

pub struct Window {
    handle: HWND,
}

#[derive(Clone, Debug)]
pub enum Key {
    P,
    Left,
    Up,
    Space,
    Escape,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    Ctrl(Box<Key>),
}

use std::thread::sleep;
use std::time::Duration;
use winapi::ctypes::c_int;
pub use Key::*;

pub fn ctrl<K: Into<Box<Key>>>(key: K) -> Key {
    Ctrl(key.into())
}

impl IntoIterator for Key {
    type Item = INPUT;
    type IntoIter = <Vec<INPUT> as IntoIterator>::IntoIter;
    #[allow(non_snake_case)]
    fn into_iter(self) -> Self::IntoIter {
        fn keydown(vk: c_int) -> INPUT {
            let mut result: INPUT = unsafe { std::mem::zeroed() };
            result.type_ = INPUT_KEYBOARD;
            unsafe {
                let info = result.u.ki_mut();
                info.wVk = vk as u16;
            }
            result
        }
        fn keyup(vk: c_int) -> INPUT {
            let mut result: INPUT = unsafe { std::mem::zeroed() };
            result.type_ = INPUT_KEYBOARD;
            unsafe {
                let info = result.u.ki_mut();
                info.wVk = vk as u16;
                info.dwFlags = KEYEVENTF_KEYUP;
            }
            result
        }
        let vk = match self {
            P => 0x50,
            Left => VK_LEFT,
            Up => VK_UP,
            Escape => VK_ESCAPE,
            Space => VK_SPACE,
            F1 => VK_F1,
            F2 => VK_F2,
            F3 => VK_F3,
            F4 => VK_F4,
            F5 => VK_F5,
            F6 => VK_F6,
            F7 => VK_F7,
            F8 => VK_F8,
            F9 => VK_F9,
            F10 => VK_F10,
            F11 => VK_F11,
            F12 => VK_F12,
            Ctrl(k) => {
                let before = keydown(VK_LCONTROL);
                let after = keyup(VK_LCONTROL);
                return std::iter::once(before)
                    .chain(k.into_iter())
                    .chain(std::iter::once(after))
                    .collect::<Vec<_>>()
                    .into_iter();
            }
        };
        vec![keydown(vk), keyup(vk)].into_iter()
    }
}

unsafe extern "system" fn has_title(win: HWND, arg: LPARAM) -> BOOL {
    let result = arg as *mut WindowSearch;
    let size = GetWindowTextLengthA(win);
    if size == 0 {
        return TRUE;
    }
    let mut raw_title = vec![0i8; size as usize + 1];
    GetWindowTextA(win, &mut raw_title[0] as LPSTR, size + 1);
    let raw_title = raw_title.iter().map(|&x| x as u8).collect::<Vec<_>>();
    let title = String::from_utf8_lossy(&raw_title[0..(size as usize)]);
    if title == (*result).target.pattern() {
        (*result).handle = Some(win);
        return FALSE;
    }
    TRUE
}

impl Window {
    fn find(target: Target) -> Window {
        let mut result: Box<WindowSearch> = Box::new(WindowSearch {
            handle: None,
            target,
        });
        unsafe {
            EnumWindows(
                Some(has_title),
                result.as_mut() as *mut WindowSearch as LPARAM,
            );
        }
        Window {
            handle: result
                .handle
                .expect(&format!("Couldn't find {:?} window", target)),
        }
    }

    pub fn overwatch() -> Window {
        Window::find(Target::Overwatch)
    }

    pub fn await_focus(&self) {
        unsafe fn is_focused(handle: HWND) -> bool {
            GetForegroundWindow() == handle
        }
        while unsafe { !is_focused(self.handle) } {
            sleep(Duration::from_millis(100));
        }
    }

    pub fn send(&self, key: &Key) {
        unsafe {
            let mut inputs: Vec<INPUT> = key.clone().into_iter().collect();
            SendInput(
                inputs.len() as u32,
                &mut inputs[0] as *mut INPUT,
                std::mem::size_of::<INPUT>() as i32,
            );
        }
    }
}
