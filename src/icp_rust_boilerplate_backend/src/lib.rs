#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Poll {
    id: u64,
    question: String,
    options: Vec<String>,
    votes: Vec<u64>,
    created_at: u64,
    updated_at: Option<u64>,
}

// A trait that must be implemented for a struct stored in stable memory
impl Storable for Poll {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Poll {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static STORAGE: RefCell<StableBTreeMap<u64, Poll, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct PollPayload {
    question: String,
    options: Vec<String>,
}

#[ic_cdk::query]
fn get_poll(id: u64) -> Result<Poll, Error> {
    match _get_poll(&id) {
        Some(poll) => Ok(poll),
        None => Err(Error::NotFound {
            msg: format!("Poll with id={} not found", id),
        }),
    }
}

#[ic_cdk::update]
fn create_poll(payload: PollPayload) -> Option<Poll> {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("cannot increment id counter");

    let poll = Poll {
        id,
        question: payload.question,
        options: payload.options.clone(),
        votes: vec![0; payload.options.len()],
        created_at: time(),
        updated_at: None,
    };

    do_insert(&poll);
    Some(poll)
}

#[ic_cdk::update]
fn vote(poll_id: u64, option_index: usize) -> Result<Poll, Error> {
    match STORAGE.with(|service| service.borrow().get(&poll_id)) {
        Some(mut poll) => {
            if option_index < poll.options.len() {
                poll.votes[option_index] += 1;
                poll.updated_at = Some(time());
                do_insert(&poll);
                Ok(poll)
            } else {
                Err(Error::InvalidVote {
                    msg: "Invalid option index".to_string(),
                })
            }
        }
        None => Err(Error::NotFound {
            msg: format!("Poll with id={} not found", poll_id),
        }),
    }
}

fn do_insert(poll: &Poll) {
    STORAGE.with(|service| service.borrow_mut().insert(poll.id, poll.clone()));
}

#[ic_cdk::update]
fn delete_poll(id: u64) -> Result<Poll, Error> {
    match STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(poll) => Ok(poll),
        None => Err(Error::NotFound {
            msg: format!("Poll with id={} not found.", id),
        }),
    }
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
    InvalidVote { msg: String },
}

// Helper function to get poll by ID
fn _get_poll(id: &u64) -> Option<Poll> {
    STORAGE.with(|service| service.borrow().get(id))
}

ic_cdk::export_candid!();
