// Windows release 构建时隐藏控制台窗口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    ai_interview_assistant_lib::run()
}
