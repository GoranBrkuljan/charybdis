use crate::common::db_session;
use charybdis::callbacks::Callbacks;
use charybdis::errors::CharybdisError;
use charybdis::macros::charybdis_model;
use charybdis::operations::{Delete, DeleteWithCallbacks, Insert, InsertWithCallbacks, Update, UpdateWithCallbacks};
use charybdis::types::{Int, Text, Uuid};
use scylla::client::caching_session::CachingSession;
use std::fmt;
use std::sync::Mutex;

#[charybdis_model(
    table_name = notes,
    partition_keys = [id],
    clustering_keys = [],
    global_secondary_indexes = []
)]
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Note {
    pub id: Uuid,
    pub title: Text,
    // maintained by the callbacks: set by `before_insert`, incremented by `before_update`
    pub revision: Int,
}

impl Note {
    fn new_draft() -> Self {
        Note {
            id: Uuid::new_v4(),
            title: "draft".to_string(),
            revision: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Callback {
    BeforeInsert,
    AfterInsert,
    BeforeUpdate,
    AfterUpdate,
    BeforeDelete,
    AfterDelete,
}

#[derive(Debug)]
pub enum NoteError {
    Charybdis(CharybdisError),
    Rejected(Callback),
}

impl fmt::Display for NoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NoteError::Charybdis(error) => write!(f, "{}", error),
            NoteError::Rejected(callback) => write!(f, "Rejected by {:?}", callback),
        }
    }
}

impl std::error::Error for NoteError {}

impl From<CharybdisError> for NoteError {
    fn from(error: CharybdisError) -> Self {
        NoteError::Charybdis(error)
    }
}

/// The extension that is handed to every callback. It records which callbacks ran, together with the
/// row that was in the database at that moment, and it can make one of the callbacks fail.
#[derive(Default)]
pub struct Recorder {
    calls: Mutex<Vec<(Callback, Option<Note>)>>,
    failing: Option<Callback>,
}

impl Recorder {
    fn failing_on(callback: Callback) -> Self {
        Recorder {
            failing: Some(callback),
            ..Default::default()
        }
    }

    fn calls(&self) -> Vec<(Callback, Option<Note>)> {
        self.calls.lock().expect("recorder lock is not poisoned").clone()
    }

    async fn record(&self, callback: Callback, note: &Note, session: &CachingSession) -> Result<(), NoteError> {
        let persisted = Note::maybe_find_first_by_id(note.id).execute(session).await?;

        self.calls
            .lock()
            .expect("recorder lock is not poisoned")
            .push((callback, persisted));

        if self.failing == Some(callback) {
            return Err(NoteError::Rejected(callback));
        }

        Ok(())
    }
}

impl Callbacks for Note {
    type Extension = Recorder;
    type Error = NoteError;

    async fn before_insert(&mut self, session: &CachingSession, recorder: &Recorder) -> Result<(), NoteError> {
        recorder.record(Callback::BeforeInsert, self, session).await?;
        self.revision = 1;

        Ok(())
    }

    async fn after_insert(&mut self, session: &CachingSession, recorder: &Recorder) -> Result<(), NoteError> {
        recorder.record(Callback::AfterInsert, self, session).await
    }

    async fn before_update(&mut self, session: &CachingSession, recorder: &Recorder) -> Result<(), NoteError> {
        recorder.record(Callback::BeforeUpdate, self, session).await?;
        self.revision += 1;

        Ok(())
    }

    async fn after_update(&mut self, session: &CachingSession, recorder: &Recorder) -> Result<(), NoteError> {
        recorder.record(Callback::AfterUpdate, self, session).await
    }

    async fn before_delete(&mut self, session: &CachingSession, recorder: &Recorder) -> Result<(), NoteError> {
        recorder.record(Callback::BeforeDelete, self, session).await
    }

    async fn after_delete(&mut self, session: &CachingSession, recorder: &Recorder) -> Result<(), NoteError> {
        recorder.record(Callback::AfterDelete, self, session).await
    }
}

#[tokio::test]
async fn insert_callbacks() -> Result<(), NoteError> {
    let db_session = &db_session().await;
    let recorder = Recorder::default();
    let mut note = Note::new_draft();

    note.insert_cb(&recorder).execute(db_session).await?;

    // `before_insert` changed the model before it was written
    assert_eq!(note.revision, 1);
    assert_eq!(Note::find_by_id(note.id).execute(db_session).await?, note);

    // `before_insert` ran when the row did not exist yet, `after_insert` once it was written
    assert_eq!(
        recorder.calls(),
        vec![
            (Callback::BeforeInsert, None),
            (Callback::AfterInsert, Some(note.clone()))
        ]
    );

    note.delete().execute(db_session).await?;

    Ok(())
}

#[tokio::test]
async fn update_callbacks() -> Result<(), NoteError> {
    let db_session = &db_session().await;
    let recorder = Recorder::default();
    let draft = Note::new_draft();

    draft.insert().execute(db_session).await?;

    let mut note = Note {
        title: "final".to_string(),
        ..draft.clone()
    };
    note.update_cb(&recorder).execute(db_session).await?;

    // `before_update` changed the model before it was written
    assert_eq!(note.revision, draft.revision + 1);
    assert_eq!(Note::find_by_id(note.id).execute(db_session).await?, note);

    // `before_update` still saw the old row, `after_update` the new one
    assert_eq!(
        recorder.calls(),
        vec![
            (Callback::BeforeUpdate, Some(draft)),
            (Callback::AfterUpdate, Some(note.clone()))
        ]
    );

    note.delete().execute(db_session).await?;

    Ok(())
}

#[tokio::test]
async fn delete_callbacks() -> Result<(), NoteError> {
    let db_session = &db_session().await;
    let recorder = Recorder::default();
    let mut note = Note::new_draft();

    note.insert().execute(db_session).await?;
    note.delete_cb(&recorder).execute(db_session).await?;

    assert_eq!(Note::maybe_find_first_by_id(note.id).execute(db_session).await?, None);

    // `before_delete` ran while the row was still there, `after_delete` once it was gone
    assert_eq!(
        recorder.calls(),
        vec![
            (Callback::BeforeDelete, Some(note.clone())),
            (Callback::AfterDelete, None)
        ]
    );

    Ok(())
}

#[tokio::test]
async fn failing_before_callback_cancels_the_operation() -> Result<(), NoteError> {
    let db_session = &db_session().await;
    let persisted = Note::new_draft();

    persisted.insert().execute(db_session).await?;

    // insert: nothing is written
    let recorder = Recorder::failing_on(Callback::BeforeInsert);
    let mut new_note = Note::new_draft();
    let result = new_note.insert_cb(&recorder).execute(db_session).await;

    assert!(matches!(result, Err(NoteError::Rejected(Callback::BeforeInsert))));
    assert_eq!(
        Note::maybe_find_first_by_id(new_note.id).execute(db_session).await?,
        None
    );
    assert_eq!(recorder.calls(), vec![(Callback::BeforeInsert, None)]);

    // update: the row keeps its old values
    let recorder = Recorder::failing_on(Callback::BeforeUpdate);
    let mut changed = Note {
        title: "final".to_string(),
        ..persisted.clone()
    };
    let result = changed.update_cb(&recorder).execute(db_session).await;

    assert!(matches!(result, Err(NoteError::Rejected(Callback::BeforeUpdate))));
    assert_eq!(Note::find_by_id(persisted.id).execute(db_session).await?, persisted);
    assert_eq!(
        recorder.calls(),
        vec![(Callback::BeforeUpdate, Some(persisted.clone()))]
    );

    // delete: the row is still there
    let recorder = Recorder::failing_on(Callback::BeforeDelete);
    let mut doomed = persisted.clone();
    let result = doomed.delete_cb(&recorder).execute(db_session).await;

    assert!(matches!(result, Err(NoteError::Rejected(Callback::BeforeDelete))));
    assert_eq!(Note::find_by_id(persisted.id).execute(db_session).await?, persisted);
    assert_eq!(
        recorder.calls(),
        vec![(Callback::BeforeDelete, Some(persisted.clone()))]
    );

    persisted.delete().execute(db_session).await?;

    Ok(())
}

#[tokio::test]
async fn failing_after_callback_keeps_the_write() -> Result<(), NoteError> {
    let db_session = &db_session().await;

    // insert: the error is returned, but the row has already been written
    let recorder = Recorder::failing_on(Callback::AfterInsert);
    let mut note = Note::new_draft();
    let result = note.insert_cb(&recorder).execute(db_session).await;

    assert!(matches!(result, Err(NoteError::Rejected(Callback::AfterInsert))));
    assert_eq!(Note::find_by_id(note.id).execute(db_session).await?, note);

    // update: the row has already been changed
    let recorder = Recorder::failing_on(Callback::AfterUpdate);
    note.title = "final".to_string();
    let result = note.update_cb(&recorder).execute(db_session).await;

    assert!(matches!(result, Err(NoteError::Rejected(Callback::AfterUpdate))));
    assert_eq!(Note::find_by_id(note.id).execute(db_session).await?, note);

    // delete: the row is already gone
    let recorder = Recorder::failing_on(Callback::AfterDelete);
    let result = note.delete_cb(&recorder).execute(db_session).await;

    assert!(matches!(result, Err(NoteError::Rejected(Callback::AfterDelete))));
    assert_eq!(Note::maybe_find_first_by_id(note.id).execute(db_session).await?, None);

    Ok(())
}

#[tokio::test]
async fn operations_without_cb_skip_the_callbacks() -> Result<(), NoteError> {
    let db_session = &db_session().await;
    let draft = Note::new_draft();

    // `before_insert` would have set the revision to 1
    draft.insert().execute(db_session).await?;
    assert_eq!(Note::find_by_id(draft.id).execute(db_session).await?, draft);

    // `before_update` would have incremented the revision
    let changed = Note {
        title: "final".to_string(),
        ..draft.clone()
    };
    changed.update().execute(db_session).await?;
    assert_eq!(Note::find_by_id(draft.id).execute(db_session).await?, changed);

    changed.delete().execute(db_session).await?;

    Ok(())
}
