use std::any::{Any, TypeId};
use std::collections::HashSet;
use std::ptr::NonNull;
use rustc_hash::FxHashMap;
use crate::systems::entry::SendSyncNonNull;
use crate::systems::variadics::OwnershipType;
// UNSAFE STUFF AND RAW POINTER SHIT
// like 2 lines of code

pub trait DisjointedAccess {
    fn fetch_args(&mut self, key_pointers: &Vec<(OwnershipType, TypeId)>) -> Option<Vec<SendSyncNonNull>>;
    /// # Safety
    /// Every key must be present in the map and no key may appear twice, otherwise the returned
    /// pointers alias or dangle. The pointers stay valid only until the map is mutated in a way
    /// that moves or drops the resources they point at.
    unsafe fn fetch_args_unchecked(&mut self, key_pointers: &Vec<(OwnershipType, TypeId)>) -> Vec<SendSyncNonNull>;
}

impl DisjointedAccess for FxHashMap<TypeId, Box<dyn Any>> {
    // Safety checks to validate inputs with some runtime overhead
    fn fetch_args(&mut self, key_pointers: &Vec<(OwnershipType, TypeId)>) -> Option<Vec<SendSyncNonNull>> {
        let mut data = HashSet::new();
        // For each one, check for duplicates, or any invalid resources
        // This checks locally, as in within the function, not across functions
        // Note for future optimization,
        // this check can be done while importing the function to avoid runtime safety checks
        // next time extend rather than do seperate loops
        for (_,key) in key_pointers {
            if !data.insert(key) || !self.contains_key(key) {
                return None
            }
        }
        // The keys were just validated as present and disjoint
        Some(unsafe { self.fetch_args_unchecked(key_pointers) })
    }
    // Good to use after you've confirmed your schedule are valid
    unsafe fn fetch_args_unchecked(&mut self, key_pointers: &Vec<(OwnershipType, TypeId)>) -> Vec<SendSyncNonNull> {
        let mut data = Vec::new();
        // Remove owned values beforehand because the memory location is affected
        for (ownership, key) in key_pointers {
            match ownership {
                OwnershipType::Mut => unsafe {
                    let resource = SendSyncNonNull::new(
                        NonNull::new_unchecked(self.get_mut(key).unwrap().as_mut() as *mut dyn Any as *mut u8)
                    );
                    data.push(resource)
                }
                OwnershipType::Ref => unsafe {
                    let resource = SendSyncNonNull::new(
                        NonNull::new_unchecked(self.get(key).unwrap().as_ref() as *const dyn Any as *mut u8)
                    );
                    data.push(resource)
                }
            }
        }
        data
    }
}