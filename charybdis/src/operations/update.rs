use crate::callbacks::{Callbacks, UpdateAction};
use crate::model::Model;
use crate::query::{CharybdisCbQuery, CharybdisQuery, ModelMutation, QueryValue};

pub trait Update: Model {
    fn update(&self) -> CharybdisQuery<'_, Self, Self, ModelMutation> {
        CharybdisQuery::new(Self::UPDATE_QUERY, QueryValue::Model(self))
    }
}

impl<M: Model> Update for M {}

pub trait UpdateWithCallbacks<'a>: Callbacks {
    fn update_cb(&'a mut self, extension: &'a Self::Extension) -> CharybdisCbQuery<'a, Self, UpdateAction<Self>, Self> {
        CharybdisCbQuery::new(Self::UPDATE_QUERY, self, extension)
    }
}

impl<M: Callbacks> UpdateWithCallbacks<'_> for M {}
