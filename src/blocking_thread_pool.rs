use std::cell::UnsafeCell;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::SyncSender;
use std::thread;
use std::thread::JoinHandle;

type Task = Box<dyn FnOnce() + Send + 'static>;

pub enum WorkerMessage {
    Task(Task),
    Terminate,
}

pub struct Worker {
    pub id: u32,
    pub sender: SyncSender<WorkerMessage>,
    pub receiver: Arc<Mutex<Receiver<WorkerMessage>>>,
    pub thread: Option<JoinHandle<()>>,
    pub exited_flag: bool,
    pub busy: bool,
}

impl Worker {
    pub fn new(id: u32, sender: SyncSender<WorkerMessage>, receiver: Arc<Mutex<Receiver<WorkerMessage>>>) -> Arc<UnsafeCell<Worker>> {
        let worker = Arc::new(UnsafeCell::new(Worker {
            id,
            sender,
            receiver,
            exited_flag: false,
            thread: None,
            busy: false,
        }));

        unsafe {
            let worker2 = &mut *worker.get();
            (&mut *worker.get()).thread = Option::Some(thread::spawn(move || worker2.run()));
        }
        
        worker
    }

    pub fn run(&mut self) {
        while !self.exited_flag {
            let msg = self.receiver.lock().unwrap().recv().unwrap();

            // println!(".{}", self.id);
            
            match msg {
                WorkerMessage::Task(task) => {
                    self.busy = true;
                    task();
                    self.busy = false;
                }

                WorkerMessage::Terminate => {
                    break;
                }
            }
        }

        // println!("t {} exit", self.id);

        self.thread = None;
    }

    // pub fn terminate(&mut self) {
    //     self.exited_flag = true;

    //     if self.is_terminated() {
    //         panic!("thread {} has terminated.", self.id);
    //     }
        
    //     if !self.busy {
    //         self.sender.send(WorkerMessage::Terminate).unwrap();
    //     }
    // }

    pub fn is_terminated(&self) -> bool {
        self.thread.is_none()
    }

    pub fn is_busy(&self) -> bool {
        self.busy
    }

    pub fn wait(&mut self) {
        if self.is_terminated() {
            return;
        }

        self.thread.take().unwrap().join().unwrap();
    }
}

pub struct BlockingThreadPool {
    workers: Vec<Arc<UnsafeCell<Worker>>>,
    sender: mpsc::SyncSender<WorkerMessage>,
    is_terminated: bool,
}

impl BlockingThreadPool {
    pub fn new(size: usize) -> BlockingThreadPool {
        assert!(size > 0);

        let (sender, receiver) = mpsc::sync_channel(0);
        let mut workers = Vec::with_capacity(size);

        let receiver = Arc::new(Mutex::new(receiver));
        for id in 0..size {
            workers.push(Worker::new(id as u32, sender.clone(), receiver.clone()));
        }

        BlockingThreadPool { workers, sender, is_terminated: false }
    }

    pub fn execute<F>(&self, fun: F) where F : FnOnce() + Send + 'static, {
        if self.is_terminated {
            panic!("dispatching task after thread pool was closed.");
        }

        self.sender.send(WorkerMessage::Task(Box::new(fun))).unwrap();
    }

    pub fn terminate_and_wait(&mut self) {
        self.is_terminated = true;

        for _worker in &self.workers {
            self.sender.send(WorkerMessage::Terminate).unwrap();
        }

        if true {
            for worker in &self.workers {
                unsafe {
                    let worker =  &mut *worker.get();
    
                    // println!("wait   end {}", worker.id);
                    worker.wait();
                    // println!("thread end {}", worker.id);
                }
            }
        }
    }

    pub fn size(&self) -> u32 {
        self.workers.len() as u32
    }
}

impl Drop for BlockingThreadPool {
    fn drop(&mut self) {
        if !self.is_terminated {
            self.terminate_and_wait();
        }
    }
}