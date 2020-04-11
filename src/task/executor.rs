use super::{Task, TaskId};
use alloc::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
    task::Wake,
};
use core::task::{Context, Poll, Waker};
use crossbeam_queue::ArrayQueue;

struct TaskWaker {
    task_id: TaskId,
    wake_queue: Arc<ArrayQueue<TaskId>>,
}

impl TaskWaker {
    fn wake_task(&self) {
        self.wake_queue
            .push(self.task_id)
            .expect("wake_queue is full");
    }
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_task();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}

#[derive(Clone)]
pub struct Spawner {
    spawned_tasks: Arc<ArrayQueue<Task>>,
}

impl Spawner {
    pub fn spawn(&self, task: Task) {
        self.spawned_tasks.push(task).expect("why")
    }
}

pub struct Executor {
    task_queue: VecDeque<Task>,
    spawned_tasks: Arc<ArrayQueue<Task>>,
    waiting_tasks: BTreeMap<TaskId, Task>,
    wake_queue: Arc<ArrayQueue<TaskId>>,
    waker_cache: BTreeMap<TaskId, Waker>,
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            task_queue: VecDeque::new(),
            spawned_tasks: Arc::new(ArrayQueue::new(100)),
            waiting_tasks: BTreeMap::new(),
            wake_queue: Arc::new(ArrayQueue::new(100)),
            waker_cache: BTreeMap::new(),
        }
    }

    pub fn spawn(&mut self, task: Task) {
        self.task_queue.push_back(task)
    }

    pub fn spawner(&self) -> Spawner {
        Spawner {
            spawned_tasks: self.spawned_tasks.clone(),
        }
    }

    pub fn run(&mut self) -> ! {
        loop {
            self.wake_tasks();
            self.run_ready_tasks();
            self.sleep_if_idle(); // Getting here means that there are no tasks left in `task_queue`
        }
    }

    fn sleep_if_idle(&self) {
        use x86_64::instructions::interrupts::{self, enable_interrupts_and_hlt};

        // Return early, no need to disable interrupts
        if !self.wake_queue.is_empty() {
            return;
        }

        interrupts::disable();
        // If an interrupt happened inbetween, interrupts will be enabled
        if self.wake_queue.is_empty() {
            enable_interrupts_and_hlt();
        } else {
            interrupts::enable();
        }
    }

    fn create_waker(&self, task_id: TaskId) -> Waker {
        Waker::from(Arc::new(TaskWaker {
            task_id,
            wake_queue: self.wake_queue.clone(),
        }))
    }

    fn run_ready_tasks(&mut self) {
        while let Ok(task) = self.spawned_tasks.pop() {
            self.task_queue.push_back(task);
        }
        while let Some(mut task) = self.task_queue.pop_front() {
            let task_id = task.id;
            // Create a new `Waker` if it isn't already in the cache.
            if !self.waker_cache.contains_key(&task_id) {
                self.waker_cache.insert(task_id, self.create_waker(task_id));
            }

            let waker = self
                .waker_cache
                .get(&task_id)
                .expect("There should be a waker with this key. I hope.");
            let mut context = Context::from_waker(waker);
            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    // Task is done, remove cached waker
                    self.waker_cache.remove(&task_id);
                }
                Poll::Pending => {
                    if self.waiting_tasks.insert(task_id, task).is_some() {
                        panic!("Task with same ID already in waiting_tasks!");
                    }
                }
            }
        }
    }

    fn wake_tasks(&mut self) {
        while let Ok(task_id) = self.wake_queue.pop() {
            if let Some(task) = self.waiting_tasks.remove(&task_id) {
                self.task_queue.push_back(task);
            }
        }
    }
}