use std::any::{Any, TypeId};
use std::collections::HashSet;
use std::fmt;
use std::ptr::NonNull;
use fxhash::FxHashMap;
use crate::systems::entry::SendSyncNonNull;
use crate::systems::variadics::OwnershipType;
// UNSAFE STUFF AND RAW POINTER SHIT
// like 2 lines of code

// Reasons a system cannot get disjointed access to the resources it asks for
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    // The same resource is requested twice by a single system, which would alias
    AliasedResource(TypeId),
    // The resource was never inserted into the resource handler
    MissingResource(TypeId),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchError::AliasedResource(id) => write!(
                f,
                "resource {id:?} is requested more than once by the same system, which would alias it"
            ),
            FetchError::MissingResource(id) => write!(
                f,
                "resource {id:?} is requested by a system but was never added to the app"
            ),
        }
    }
}

impl std::error::Error for FetchError {}

pub trait DisjointedAccess {
    fn fetch_args(&mut self, key_pointers: &Vec<(OwnershipType, TypeId)>) -> Result<Vec<SendSyncNonNull>, FetchError>;
    fn fetch_args_unchecked(&mut self, key_pointers: &Vec<(OwnershipType, TypeId)>) -> Vec<SendSyncNonNull>;
}

impl DisjointedAccess for FxHashMap<TypeId, Box<dyn Any>> {
    // Safety checks to validate inputs with some runtime overhead
    fn fetch_args(&mut self, key_pointers: &Vec<(OwnershipType, TypeId)>) -> Result<Vec<SendSyncNonNull>, FetchError> {
        let mut data = HashSet::new();
        // For each one, check for duplicates, or any invalid resources
        // This checks locally, as in within the function, not across functions
        // Note for future optimization,
        // this check can be done while importing the function to avoid runtime safety checks
        // next time extend rather than do seperate loops
        for (_,key) in key_pointers {
            if !data.insert(key) {
                return Err(FetchError::AliasedResource(*key))
            }
            if !self.contains_key(key) {
                return Err(FetchError::MissingResource(*key))
            }
        }
        Ok(self.fetch_args_unchecked(key_pointers))
    }
    // Good to use after you've confirmed your schedule are valid
    fn fetch_args_unchecked(&mut self, key_pointers: &Vec<(OwnershipType, TypeId)>) -> Vec<SendSyncNonNull> {
        let mut data = Vec::new();
        // Remove owned values beforehand because the memory location is affected
        for (ownership, key) in key_pointers {
            match ownership {
                OwnershipType::Mut => unsafe {
                    let entry = self.get_mut(key).unwrap_or_else(|| {
                        panic!("{}", FetchError::MissingResource(*key))
                    });
                    let resource = SendSyncNonNull::from(
                        NonNull::new_unchecked(entry.as_mut() as *mut dyn Any as *mut u8)
                    );
                    data.push(resource)
                }
                OwnershipType::Ref => unsafe {
                    let entry = self.get(key).unwrap_or_else(|| {
                        panic!("{}", FetchError::MissingResource(*key))
                    });
                    let resource = SendSyncNonNull::from(
                        NonNull::new_unchecked(entry.as_ref() as *const dyn Any as *mut u8)
                    );
                    data.push(resource)
                }
            }
        }
        data
    }
}