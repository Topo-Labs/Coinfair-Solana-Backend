// Timer: 每日00:00定时执行
// - 将数据库符合要求的数据过滤出来，并通过telegram通知给管理员
use chrono::Utc;
use cron::Schedule;
use server::services::Services;
use std::{str::FromStr, sync::Arc, time::Duration};
use telegram::HopeBot;
use tokio::{task, time::sleep_until};

#[derive(Clone)]
pub struct Timer {
    pub time: String,
    pub services: Services,
    pub telegram: HopeBot,
}

impl Timer {
    // "59 59 11 * * *": 每天11:59:59执行
    pub fn new(time: Option<String>, services: Services, telegram: HopeBot) -> Self {
        match time {
            Some(time) => Timer {
                time,
                services,
                telegram,
            },
            None => Timer {
                time: "59 59 11 * * *".to_string(),
                services,
                telegram,
            },
        }
    }

    pub async fn run(self: Arc<Self>) {
        println!("⏳ Timer action at {} everyday.", self.time);

        let schedule = Schedule::from_str(&self.time).unwrap(); // UTC 11:59:59

        loop {
            let now = Utc::now();
            let next_run_time = schedule.upcoming(Utc).next().unwrap();

            let duration_until_next_run = (next_run_time - now)
                .to_std()
                .unwrap_or(Duration::from_secs(0));

            sleep_until(tokio::time::Instant::now() + duration_until_next_run).await;

            task::spawn({
                let this = Arc::clone(&self);
                async move {
                    this.filter_rewards_and_notify().await;
                }
            })
            .await
            .unwrap();
        }
    }

    async fn filter_rewards_and_notify(&self) {
        let today = Utc::now().date_naive().to_string(); // 获取当前UTC日期（格式：YYYY-MM-DD）

        let rewards = self.services.reward.get_rewards_by_day(today).await;

        // println!("🚀 执行定时任务: {}", today);
        println!("{:?}", rewards);

        //TODO: Notify rewards to admin
    }
}
