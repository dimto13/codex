use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use std::fs;
use std::future::Future;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum TaskState {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct ScheduledTask {
    pub id: String,
    pub session_id: String,
    pub due_at_utc_ms: u64,
    pub prompt: String,
    pub state: TaskState,
    pub created_at_utc_ms: u64,
    pub updated_at_utc_ms: u64,
    pub last_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DueTime {
    Relative(Duration),
    AbsoluteUtcMs(u64),
}

impl DueTime {
    pub(crate) fn normalize(self, now_utc_ms: u64) -> u64 {
        match self {
            Self::Relative(duration) => now_utc_ms.saturating_add(duration.as_millis() as u64),
            Self::AbsoluteUtcMs(timestamp) => timestamp,
        }
    }
}

#[derive(Debug)]
pub(crate) struct SchedulerStore {
    path: PathBuf,
    tasks: Vec<ScheduledTask>,
}

impl SchedulerStore {
    pub(crate) fn load(codex_home: &Path) -> Result<Self> {
        let path = codex_home.join("scheduled_tasks.json");
        let mut tasks: Vec<ScheduledTask> = if path.exists() {
            serde_json::from_slice(
                &fs::read(&path).with_context(|| format!("read {}", path.display()))?,
            )
            .with_context(|| format!("parse {}", path.display()))?
        } else {
            Vec::new()
        };
        let now = now_utc_ms()?;
        for task in &mut tasks {
            if task.state == TaskState::Running {
                task.state = TaskState::Failed;
                task.last_error = Some("interrupted_by_restart".to_string());
                task.updated_at_utc_ms = now;
            }
        }
        let store = Self { path, tasks };
        store.persist()?;
        Ok(store)
    }

    pub(crate) fn create(
        &mut self,
        session_id: String,
        due: DueTime,
        prompt: String,
    ) -> Result<ScheduledTask> {
        let now = now_utc_ms()?;
        let sequence = self.tasks.len();
        let id = format!("task-{now}-{sequence}");
        let task = ScheduledTask {
            id,
            session_id,
            due_at_utc_ms: due.normalize(now),
            prompt,
            state: TaskState::Pending,
            created_at_utc_ms: now,
            updated_at_utc_ms: now,
            last_error: None,
        };
        self.tasks.push(task.clone());
        self.persist()?;
        Ok(task)
    }

    pub(crate) fn list(&self) -> &[ScheduledTask] {
        &self.tasks
    }

    pub(crate) fn get(&self, id: &str) -> Option<&ScheduledTask> {
        self.tasks.iter().find(|task| task.id == id)
    }

    pub(crate) fn cancel(&mut self, id: &str) -> Result<Option<ScheduledTask>> {
        let now = now_utc_ms()?;
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) else {
            return Ok(None);
        };
        if task.state == TaskState::Pending {
            task.state = TaskState::Cancelled;
            task.updated_at_utc_ms = now;
            self.persist()?;
        }
        Ok(self.get(id).cloned())
    }

    pub(crate) fn recoverable_due_task_ids(&self, now_utc_ms: u64) -> Vec<String> {
        let mut ids: Vec<String> = self
            .tasks
            .iter()
            .filter(|task| task.state == TaskState::Pending && task.due_at_utc_ms <= now_utc_ms)
            .map(|task| task.id.clone())
            .collect();
        ids.sort();
        ids
    }

    pub(crate) fn claim(&mut self, id: &str) -> Result<Option<ScheduledTask>> {
        let now = now_utc_ms()?;
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) else {
            return Ok(None);
        };
        if task.state != TaskState::Pending {
            return Ok(None);
        }
        task.state = TaskState::Running;
        task.updated_at_utc_ms = now;
        task.last_error = None;
        self.persist()?;
        Ok(self.get(id).cloned())
    }

    pub(crate) fn succeed(&mut self, id: &str) -> Result<Option<ScheduledTask>> {
        self.finish(id, TaskState::Succeeded, None)
    }

    pub(crate) fn fail(&mut self, id: &str, error: String) -> Result<Option<ScheduledTask>> {
        self.finish(id, TaskState::Failed, Some(error))
    }

    pub(crate) async fn dispatch_due<F, Fut>(&mut self, now: u64, mut dispatch: F) -> Result<()>
    where
        F: FnMut(ScheduledTask) -> Fut,
        Fut: Future<Output = Result<()>>,
    {
        for id in self.recoverable_due_task_ids(now) {
            let Some(task) = self.claim(&id)? else {
                continue;
            };
            match dispatch(task).await {
                Ok(()) => {
                    self.succeed(&id)?;
                }
                Err(error) => {
                    self.fail(&id, error.to_string())?;
                }
            }
        }
        Ok(())
    }

    fn finish(
        &mut self,
        id: &str,
        state: TaskState,
        last_error: Option<String>,
    ) -> Result<Option<ScheduledTask>> {
        let now = now_utc_ms()?;
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) else {
            return Ok(None);
        };
        if task.state != TaskState::Running {
            return Ok(None);
        }
        task.state = state;
        task.updated_at_utc_ms = now;
        task.last_error = last_error;
        self.persist()?;
        Ok(self.get(id).cloned())
    }

    fn persist(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
        }
        let temp_path = self.path.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(&self.tasks)?;
        fs::write(&temp_path, bytes).with_context(|| format!("write {}", temp_path.display()))?;
        fs::rename(&temp_path, &self.path)
            .with_context(|| format!("rename {} to {}", temp_path.display(), self.path.display()))?;
        Ok(())
    }
}

fn now_utc_ms() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis() as u64)
}

#[cfg(test)]
#[path = "scheduler_task_tests.rs"]
mod tests;
