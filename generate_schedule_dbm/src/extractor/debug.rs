use colored::Colorize;
use crate::extractor::schedule_extractor::{ScheduleExtractor, Bounds};

impl<'a> ScheduleExtractor<'a> {
    pub fn debug_print(&self, emoji: &str, message: &str) {
        if self.debug {
            println!("{} {}", emoji.green(), message.bright_blue());
        }
    }

    pub fn debug_error(&self, emoji: &str, message: &str) {
        if self.debug {
            println!("{} {}", emoji.red(), message.bright_red());
        }
    }

    pub fn debug_bounds(&self, clock_id: &str, bounds: &Bounds) {
        if self.debug {
            let lb_hour = bounds.lb / 60;
            let lb_min = bounds.lb % 60;
            let ub_hour = bounds.ub / 60;
            let ub_min = bounds.ub % 60;

            println!("   {} bounds: [{:02}:{:02} - {:02}:{:02}]",
                clock_id.cyan(),
                lb_hour, lb_min,
                ub_hour, ub_min
            );
        }
    }

    pub fn debug_set_time(&self, clock_id: &str, time: i32) {
        if self.debug {
            let hours = time / 60;
            let mins = time % 60;
            println!("   Set {} to {:02}:{:02}", clock_id.cyan(), hours, mins);
        }
    }
}
