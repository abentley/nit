// Copyright 2021-2022 Aaron Bentley <aaron@aaronbentley.com>
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::git::{LocalBranchName, ReferenceSpec};
use git2::{Error, ErrorClass, ErrorCode, Reference, Repository};
use std::borrow::Cow;
use std::fmt;
use std::fmt::{Display, Formatter};

pub enum RefErr {
    NotFound(Error),
    NotBranch,
    NotUtf8,
    Other(Error),
}

pub struct PrevRefErr(RefErr);

impl Display for PrevRefErr {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            formatter,
            "{}",
            match &self.0 {
                RefErr::NotFound(_) => "No previous branch.",
                RefErr::NotBranch => "Previous entry is not a branch.",
                RefErr::NotUtf8 => "Previous entry is not valid utf-8.",
                RefErr::Other(err) => return err.fmt(formatter),
            }
        )
    }
}

pub struct NextRefErr(pub RefErr);

impl Display for NextRefErr {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            formatter,
            "{}",
            match &self.0 {
                RefErr::NotFound(_) => "No next branch.",
                RefErr::NotBranch => "Next entry is not a branch.",
                RefErr::NotUtf8 => "Next entry is not valid utf-8.",
                RefErr::Other(err) => return err.fmt(formatter),
            }
        )
    }
}

impl From<Error> for RefErr {
    fn from(err: Error) -> RefErr {
        if err.class() == ErrorClass::Reference && err.code() == ErrorCode::NotFound {
            return RefErr::NotFound(err);
        }
        RefErr::Other(err)
    }
}

pub fn resolve_symbolic_reference(
    repo: &Repository,
    next_ref: &impl ReferenceSpec,
) -> Result<String, RefErr> {
    let target_ref = repo.find_reference(&next_ref.full())?;
    let Some(target_bytes) = target_ref.symbolic_target_bytes() else {
        return Err(RefErr::NotBranch);
    };
    let Ok(target) = String::from_utf8(target_bytes.to_owned()) else {
        return Err(RefErr::NotUtf8);
    };
    Ok(target)
}

#[derive(Debug)]
pub struct PipeNext {
    pub name: LocalBranchName,
}

pub trait SiblingBranch {
    type BranchError;
    fn wrap(err: RefErr) -> Self::BranchError;
}

impl SiblingBranch for PipeNext {
    type BranchError = NextRefErr;
    fn wrap(err: RefErr) -> NextRefErr {
        NextRefErr(err)
    }
}

impl From<LocalBranchName> for PipeNext {
    fn from(name: LocalBranchName) -> PipeNext {
        PipeNext { name }
    }
}

impl ReferenceSpec for PipeNext {
    fn full(&self) -> Cow<str> {
        format!("refs/pipe-next/{}", self.name.short()).into()
    }
    fn short(&self) -> Cow<str> {
        self.full()
    }
}

#[derive(Debug)]
pub struct PipePrev {
    pub name: LocalBranchName,
}

impl SiblingBranch for PipePrev {
    type BranchError = PrevRefErr;
    fn wrap(err: RefErr) -> PrevRefErr {
        PrevRefErr(err)
    }
}

impl From<LocalBranchName> for PipePrev {
    fn from(name: LocalBranchName) -> PipePrev {
        PipePrev { name }
    }
}

impl ReferenceSpec for PipePrev {
    fn full(&self) -> Cow<str> {
        format!("refs/pipe-prev/{}", self.name.short()).into()
    }
    fn short(&self) -> Cow<str> {
        self.full()
    }
}

#[derive(Debug)]
pub enum LinkFailure<'repo> {
    BranchValidationError(BranchValidationError<'repo>),
    PrevReferenceExists,
    NextReferenceExists,
    SameReference,
}

impl Display for LinkFailure<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(
            formatter,
            "{}",
            match &self {
                LinkFailure::BranchValidationError(err) => {
                    return write!(formatter, "{:?}", err);
                }
                LinkFailure::PrevReferenceExists => "Previous reference exists",
                LinkFailure::NextReferenceExists => "NextReferenceExists",
                LinkFailure::SameReference => "Previous and next are the same.",
            }
        )
    }
}

pub enum BranchValidationError<'repo> {
    NotLocalBranch(&'repo Reference<'repo>),
    NotUtf8(&'repo Reference<'repo>),
}

impl fmt::Debug for BranchValidationError<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match &self {
            BranchValidationError::NotLocalBranch(_) => write!(formatter, "Not local branch"),
            BranchValidationError::NotUtf8(_) => write!(formatter, "Not UTF-8"),
        }
    }
}

impl<'repo> From<BranchValidationError<'repo>> for LinkFailure<'repo> {
    fn from(err: BranchValidationError<'repo>) -> LinkFailure<'repo> {
        LinkFailure::BranchValidationError(err)
    }
}

impl<'repo> TryFrom<&'repo Reference<'repo>> for LocalBranchName {
    type Error = BranchValidationError<'repo>;
    fn try_from(reference: &'repo Reference) -> Result<Self, BranchValidationError<'repo>> {
        if !reference.is_branch() {
            return Err(BranchValidationError::NotLocalBranch(reference));
        }
        let Some(name) = reference.shorthand() else {
            return Err(BranchValidationError::NotUtf8(reference))
        };
        Ok(LocalBranchName {
            name: name.to_owned(),
        })
    }
}

pub fn link_branches<'repo>(
    repo: &Repository,
    prev: &'repo Reference,
    next: &'repo Reference,
) -> Result<(), LinkFailure<'repo>> {
    if prev.name_bytes() == next.name_bytes() {
        return Err(LinkFailure::SameReference);
    }
    let next_name: LocalBranchName = (next.try_into())?;
    let prev_reference = PipePrev::from(next_name.clone());
    if repo.find_reference(&prev_reference.full()).is_ok() {
        return Err(LinkFailure::PrevReferenceExists);
    }
    let prev_name: LocalBranchName = (prev.try_into())?;
    let next_reference = PipeNext::from(prev_name.clone());
    if repo.find_reference(&next_reference.full()).is_ok() {
        return Err(LinkFailure::NextReferenceExists);
    }
    repo.reference_symbolic(
        &next_reference.full(),
        &next_name.full(),
        false,
        "Connecting branches",
    )
    .unwrap();
    repo.reference_symbolic(
        &prev_reference.full(),
        &prev_name.full(),
        false,
        "Connecting branches",
    )
    .unwrap();
    Ok(())
}
