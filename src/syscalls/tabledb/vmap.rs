/*
 * Copyright (C) 2026 Cybernetica
 *
 * Research/Commercial License Usage
 * Licensees holding a valid Research License or Commercial License
 * for the Software may use this file according to the written
 * agreement between you and Cybernetica.
 *
 * GNU General Public License Usage
 * Alternatively, this file may be used under the terms of the GNU
 * General Public License version 3.0 as published by the Free Software
 * Foundation and appearing in the file LICENSE included in the
 * packaging of this file.  Please review the following information to
 * ensure the GNU General Public License version 3.0 requirements will be
 * met: http://www.gnu.org/copyleft/gpl-3.0.html.
 *
 * For further information, please contact us at sharemind@cyber.ee.
 */

use std::{
    ffi::{CStr, CString},
    ptr,
};

use crate::{
    common::container::PointerVector,
    ffi::{
        bindings::{
            SharemindCodeBlock, SharemindDataStore,
            SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_OK,
            SharemindModuleApi0x1SyscallContext, SharemindTdbString, SharemindTdbType,
            SharemindTdbValue, SharemindTdbVectorMap, SharemindTdbVectorMapError,
            SharemindTdbVectorMapError__TDB_VECTOR_MAP_OK,
        },
        vm::syscall::{
            data_store::get_data_store,
            error::{SyscallError, SyscallErrorKind},
            find_syscall,
        },
    },
    syserr,
};

// This struct provides the functionality of the `TdbVectorMapUtil` module
// facility from mod_tabledb in rust.
//
// In mod_tabledb the `TdbVectorMapUtil` is a stateless helper class that
// implements the `SharemindTdbVectorMapUtil` interface and is passed to
// tabledb submodules as a module facility. It creates vector maps by
// instantiating the `TdbVectorMap` class (implements `SharemindTdbVectorMap`)
// and storing it in a data store together with a deleter. Afterwards, vector
// maps can be retrieved or deleted from data store directly, but
// `TdbVectorMapUtil` provides convenience methods for that.
//
// Tabledb syscalls implemented in rust runtime circumvent the tabledb submodule
// system, and therefore cannot access module facilities offered by tabledb.
// Thus, we need to simulate the `TdbVectorMapUtil` module facility in rust.
struct TdbVectorMapUtil {}

impl TdbVectorMapUtil {
    // Creates a vector map in `mod_tabledb` using the `tdb_vmap_new` syscall.
    // Retrieves and returns the freshly created vector map using `Self::get_map`.
    pub fn new_map<'a>(
        ctx: &'a SharemindModuleApi0x1SyscallContext,
        ds: &'a mut SharemindDataStore,
    ) -> Result<&'a mut SharemindTdbVectorMap, SyscallError> {
        // Create a vector map in `mod_tabledb`
        let syscall_wrapper = find_syscall(ctx, c"tdb_vmap_new")?;
        let syscall_callable = syscall_wrapper
            .callable
            .expect("Syscall wrapper callable must be set");

        let mut return_value = [
            SharemindCodeBlock { uint64: [0] }, // vmap_id
        ];

        let ret = unsafe {
            syscall_callable(
                ptr::null_mut(),           // args
                0,                         // num_args
                ptr::null(),               // refs
                ptr::null(),               // crefs
                return_value.as_mut_ptr(), // returnValue
                ctx as *const _ as *mut _, // ctx, TODO: make ctx mut instead?
            )
        };
        if ret != SharemindModuleApi0x1Error_SHAREMIND_MODULE_API_0x1_OK {
            let err_kind = SyscallErrorKind::from(ret);
            let reason = format!("unable to create new vmap: {err_kind}");
            return Err(syserr!(err_kind, reason));
        }

        // Retrieve the freshly created vector map
        let vmap_id: u64 = unsafe { return_value[0].uint64[0] };
        Ok(Self::get_map(ds, vmap_id).expect("Created vector map shall exist"))
    }

    pub fn delete_map(ds: &mut SharemindDataStore, vmap_id: u64) -> bool {
        ds.remove(&CString::new(vmap_id.to_string()).unwrap())
    }

    pub fn get_map(
        ds: &mut SharemindDataStore,
        vmap_id: u64,
    ) -> Option<&mut SharemindTdbVectorMap> {
        ds.get::<SharemindTdbVectorMap>(&CString::new(vmap_id.to_string()).unwrap())
    }
}

pub(crate) fn get_vector_maps_datastore(
    ctx: &SharemindModuleApi0x1SyscallContext,
) -> Result<&mut SharemindDataStore, SyscallError> {
    get_data_store(ctx, c"mod_tabledb/vector_maps")
}

/// TODO: Refactor to fix the `clippy::mut_from_ref` lint.
/// See comment for SharemindModuleApi0x1SyscallContext::get_process_facility()
/// in runtime/src/ffi/vm/syscall/interface.rs
#[allow(clippy::mut_from_ref)]
pub(crate) fn new_vector_map(
    ctx: &SharemindModuleApi0x1SyscallContext,
) -> Result<(&mut SharemindTdbVectorMap, u64), SyscallError> {
    let vmap_ds = get_vector_maps_datastore(ctx)?;

    let vmap = TdbVectorMapUtil::new_map(ctx, vmap_ds)?;
    let get_id_fn = vmap
        .get_id
        .expect("Function 'SharemindTdbVectorMap::get_id' shall be set");
    let vmap_id = unsafe { get_id_fn(vmap) };
    Ok((vmap, vmap_id))
}

pub(crate) fn get_vector_map(
    ctx: &SharemindModuleApi0x1SyscallContext,
    vmap_id: u64,
) -> Result<&mut SharemindTdbVectorMap, SyscallError> {
    let vmap_ds = get_vector_maps_datastore(ctx)?;

    TdbVectorMapUtil::get_map(vmap_ds, vmap_id).ok_or_else(|| {
        syserr!(
            SyscallErrorKind::GeneralError,
            "unable to retrieve the vector map"
        )
    })
}

pub(crate) fn delete_vector_map(
    ctx: &SharemindModuleApi0x1SyscallContext,
    vmap_id: u64,
) -> Result<bool, SyscallError> {
    let vmap_ds = get_vector_maps_datastore(ctx)?;
    Ok(TdbVectorMapUtil::delete_map(vmap_ds, vmap_id))
}

pub(crate) trait PtrArrayAlloc {
    type Object;

    /// Allocate a new array of objects.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `num_elements` is a valid number of elements
    /// for the allocation, and that the returned raw pointer is properly managed
    /// to avoid memory leaks or use-after-free errors.
    unsafe fn new_ptrarray(num_elements: usize) -> *mut *mut Self::Object;

    /// # Safety
    ///
    /// The caller must pass a raw pointer that was previously obtained from
    /// `new_ptrarray`. Using an invalid pointer or a pointer that has already
    /// been freed can lead to undefined behavior. Additionally, the caller must
    /// ensure that no other code is accessing the memory pointed to by the array
    /// while it is being deleted.
    unsafe fn delete_ptrarray(ptr: *mut *mut Self::Object);
}

impl PtrArrayAlloc for SharemindTdbValue {
    type Object = SharemindTdbValue;

    unsafe fn new_ptrarray(num_elements: usize) -> *mut *mut SharemindTdbValue {
        crate::ffi::bindings::SharemindTdbValue_new_ptrarray(num_elements)
    }
    unsafe fn delete_ptrarray(ptr: *mut *mut SharemindTdbValue) {
        crate::ffi::bindings::SharemindTdbValue_delete_ptrarray(ptr)
    }
}

impl PtrArrayAlloc for SharemindTdbString {
    type Object = SharemindTdbString;

    unsafe fn new_ptrarray(num_elements: usize) -> *mut *mut SharemindTdbString {
        crate::ffi::bindings::SharemindTdbString_new_ptrarray(num_elements)
    }
    unsafe fn delete_ptrarray(ptr: *mut *mut SharemindTdbString) {
        crate::ffi::bindings::SharemindTdbString_delete_ptrarray(ptr)
    }
}

impl PtrArrayAlloc for SharemindTdbType {
    type Object = SharemindTdbType;

    unsafe fn new_ptrarray(num_elements: usize) -> *mut *mut SharemindTdbType {
        crate::ffi::bindings::SharemindTdbType_new_ptrarray(num_elements)
    }
    unsafe fn delete_ptrarray(ptr: *mut *mut SharemindTdbType) {
        crate::ffi::bindings::SharemindTdbType_delete_ptrarray(ptr)
    }
}

/// A trait for working with vectors of objects in vector maps.
pub(crate) trait VmapVector {
    type Object;

    /// Sets a vector map to contain the provided vector of objects.
    ///
    /// # Safety
    ///
    /// The `ptr_arr` must be a valid mutable pointer to an array of pointers
    /// to `Self::Object`, e.g. created by PtrArrayAlloc::new_ptrarray().
    /// The ownership of the array and of the pointed elements is transferred to
    /// the vector map. The method is unsafe because the raw pointers may be
    /// accessed/dereferenced by it or by a consumer at a later time and rust
    /// cannot guarantee the pointers are valid.
    unsafe fn set_vmap_vector(
        vmap: &mut SharemindTdbVectorMap,
        vec_name: &CStr,
        ptr_arr: *mut *mut Self::Object,
        num_elements: usize,
    ) -> SharemindTdbVectorMapError;

    /// Retrieves a vector of objects from the vector map.
    ///
    /// # Safety
    ///
    /// The `ptr_arr` must be a valid mutable pointer to an array of pointers to
    /// `Self::Object`. Upon success, this function will set this pointer to
    /// point to the memory where the vector elements are stored. The ownership
    /// of the returned array and objects is NOT transferred to the caller.
    /// The method is unsafe because rust cannot guarantee that the raw pointers
    /// returned from the vector map are valid.
    unsafe fn get_vmap_vector(
        vmap: &mut SharemindTdbVectorMap,
        vec_name: &CStr,
        ptr_arr: &mut *mut *mut Self::Object,
        num_elements: &mut usize,
    ) -> SharemindTdbVectorMapError;
}

impl VmapVector for SharemindTdbValue {
    type Object = SharemindTdbValue;

    unsafe fn set_vmap_vector(
        vmap: &mut SharemindTdbVectorMap,
        vec_name: &CStr,
        ptr_arr: *mut *mut Self::Object,
        num_elements: usize,
    ) -> SharemindTdbVectorMapError {
        vmap.set_value_vector.unwrap()(vmap, vec_name.as_ptr(), ptr_arr, num_elements)
    }

    unsafe fn get_vmap_vector(
        vmap: &mut SharemindTdbVectorMap,
        vec_name: &CStr,
        ptr_arr: &mut *mut *mut Self::Object,
        num_elements: &mut usize,
    ) -> SharemindTdbVectorMapError {
        vmap.get_value_vector.unwrap()(vmap, vec_name.as_ptr(), ptr_arr, num_elements)
    }
}

impl VmapVector for SharemindTdbString {
    type Object = SharemindTdbString;

    unsafe fn set_vmap_vector(
        vmap: &mut SharemindTdbVectorMap,
        vec_name: &CStr,
        ptr_arr: *mut *mut Self::Object,
        num_elements: usize,
    ) -> SharemindTdbVectorMapError {
        vmap.set_string_vector.unwrap()(vmap, vec_name.as_ptr(), ptr_arr, num_elements)
    }

    unsafe fn get_vmap_vector(
        vmap: &mut SharemindTdbVectorMap,
        vec_name: &CStr,
        ptr_arr: &mut *mut *mut Self::Object,
        num_elements: &mut usize,
    ) -> SharemindTdbVectorMapError {
        vmap.get_string_vector.unwrap()(vmap, vec_name.as_ptr(), ptr_arr, num_elements)
    }
}

impl VmapVector for SharemindTdbType {
    type Object = SharemindTdbType;

    unsafe fn set_vmap_vector(
        vmap: &mut SharemindTdbVectorMap,
        vec_name: &CStr,
        ptr_arr: *mut *mut Self::Object,
        num_elements: usize,
    ) -> SharemindTdbVectorMapError {
        vmap.set_type_vector.unwrap()(vmap, vec_name.as_ptr(), ptr_arr, num_elements)
    }

    unsafe fn get_vmap_vector(
        vmap: &mut SharemindTdbVectorMap,
        vec_name: &CStr,
        ptr_arr: &mut *mut *mut Self::Object,
        num_elements: &mut usize,
    ) -> SharemindTdbVectorMapError {
        vmap.get_type_vector.unwrap()(vmap, vec_name.as_ptr(), ptr_arr, num_elements)
    }
}

impl SharemindTdbVectorMap {
    /// Retrieves a slice of objects from the vector map with the specified name.
    ///
    /// # Returns
    ///
    /// * `Ok(&'a [*mut T])` - A slice containing pointers to the objects in the vector, where `T` is the type of object stored in the vector.
    /// * `Err(SyscallError)` - An error if the objects could not be retrieved from the vector map.
    ///
    /// # Safety
    ///
    /// The method is unsafe because rust cannot guarantee that the returned raw
    /// pointers are valid.
    pub(crate) unsafe fn get_objects<'a, T>(
        &mut self,
        vec_name: &CStr,
    ) -> Result<&'a [*mut T], SyscallError>
    where
        T: VmapVector<Object = T>,
    {
        let vec_name_str = vec_name
            .to_str()
            .map_err(|e| syserr!(SyscallErrorKind::InvalidCall, "invalid vector name", e))?;

        let mut count: usize = 0;
        let mut object_ptr_arr: *mut *mut T = ptr::null_mut();
        if T::get_vmap_vector(self, vec_name, &mut object_ptr_arr, &mut count)
            != SharemindTdbVectorMapError__TDB_VECTOR_MAP_OK
        {
            return Err(syserr!(
                SyscallErrorKind::GeneralError,
                format!("failed to get the \"{vec_name_str}\" vector from vector map")
            ));
        }
        assert!(
            !object_ptr_arr.is_null(),
            "object array pointer must not be null"
        );
        Ok(std::slice::from_raw_parts(object_ptr_arr, count))
    }

    /// Store a vector of `SharemindTdb*` objects in a vector map.
    ///
    /// Takes ownership of the inserted objects and transfers it to the vector map.
    ///
    /// # Safety
    ///
    /// This method is unsafe because it receives the raw pointers to objects
    /// and passes them to the vector map from where the pointers may be
    /// derefrenced at some point. Rust cannot guarantee the validity of the
    /// pointed data.
    pub(crate) unsafe fn store_objects<T>(
        &mut self,
        vec_name: &CStr,
        mut objects: PointerVector<T>,
    ) -> Result<(), SyscallError>
    where
        T: PtrArrayAlloc<Object = T> + VmapVector<Object = T>,
    {
        let vec_name_str = vec_name
            .to_str()
            .map_err(|e| syserr!(SyscallErrorKind::InvalidCall, "invalid vector name", e))?;

        // Make a copy of the object pointers for `object_ptrarr_consumer` to consume.
        let object_ptrarr = T::new_ptrarray(objects.len());
        if object_ptrarr.is_null() {
            return Err(syserr!(
                SyscallErrorKind::OutOfMemory,
                format!("failed to allocate the \"{}\" vector", vec_name_str)
            ));
        }
        ptr::copy_nonoverlapping(objects.as_ptr(), object_ptrarr, objects.len());

        // Register cleanup for the copied pointer array.
        let object_ptrarr_cleanup = scopeguard::guard((), |_| {
            T::delete_ptrarray(object_ptrarr);
        });

        // Set the object vector. Vmap takes ownership of the object ptr array and its elements.
        if T::set_vmap_vector(self, vec_name, object_ptrarr, objects.len())
            != SharemindTdbVectorMapError__TDB_VECTOR_MAP_OK
        {
            return Err(syserr!(
                SyscallErrorKind::GeneralError,
                format!("failed to set the \"{vec_name_str}\" vector in vector map")
            ));
        }

        // The vmap has consumed the object ptr array and the pointed objects.
        objects.release_ownership(); // disarm cleanup for objects
        scopeguard::ScopeGuard::into_inner(object_ptrarr_cleanup); // disarm cleanup for object ptr array

        Ok(())
    }
}

/// Perform an action with a new vector map and return the vector map ID.
pub(crate) fn action_with_new_vector_map(
    ctx: &SharemindModuleApi0x1SyscallContext,
    action_func: impl FnOnce(&mut SharemindTdbVectorMap) -> Result<(), SyscallError>,
) -> Result<u64, SyscallError> {
    // Create new vector map
    let (vmap, vmap_id) = new_vector_map(ctx)?;

    // Register cleanup for the vector map
    let vmap_cleanup = scopeguard::guard((), |_| {
        if !delete_vector_map(ctx, vmap_id).unwrap_or(false) {
            tracing::error!("Error while cleaning up the vector map.");
        }
    });

    action_func(vmap)?;

    // Action was successful, disarm vmap cleanup
    scopeguard::ScopeGuard::into_inner(vmap_cleanup);

    Ok(vmap_id)
}
