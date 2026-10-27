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

use std::ops::{Deref, DerefMut, Index};

type PointerDeleter<T> = unsafe extern "C" fn(*mut T);

// Ownership-aware vector container for raw pointers.
// Deletes the contained values when dropped and ownership is assumed (i.e. deleter exists).
#[derive(Debug)]
pub struct PointerVector<T> {
    elements: Vec<*mut T>,
    deleter: Option<PointerDeleter<T>>,
}

impl<T> PointerVector<T> {
    pub fn new(capacity: usize, deleter: Option<PointerDeleter<T>>) -> Self {
        Self {
            elements: Vec::with_capacity(capacity),
            deleter,
        }
    }

    pub fn from_vec(vec: Vec<*mut T>, deleter: Option<PointerDeleter<T>>) -> Self {
        Self {
            elements: vec,
            deleter,
        }
    }

    pub fn release_ownership(&mut self) {
        self.deleter = None;
    }

    // Expose some useful vector methods

    pub fn len(&self) -> usize {
        self.elements.len()
    }

    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.elements.capacity()
    }

    pub fn push(&mut self, value: *mut T) {
        self.elements.push(value);
    }

    pub fn pop(&mut self) -> Option<*mut T> {
        self.elements.pop()
    }

    pub fn get(&self, index: usize) -> Option<*mut T> {
        self.elements.get(index).cloned()
    }

    pub fn as_ptr(&self) -> *const *mut T {
        self.elements.as_ptr()
    }
}

impl<T> Drop for PointerVector<T> {
    fn drop(&mut self) {
        if let Some(deleter) = &self.deleter {
            for &ptr in &self.elements {
                if !ptr.is_null() {
                    unsafe {
                        deleter(ptr);
                    }
                }
            }
        }
    }
}

// Allow the container to behave like a slice of raw pointers (&[*mut T])
impl<T> Deref for PointerVector<T> {
    type Target = [*mut T];

    fn deref(&self) -> &Self::Target {
        &self.elements
    }
}

// Allow mutable dereferencing (&mut [*mut T])
impl<T> DerefMut for PointerVector<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.elements
    }
}

// Allow for iteration by reference
impl<'a, T> IntoIterator for &'a PointerVector<T> {
    type Item = &'a *mut T;
    type IntoIter = std::slice::Iter<'a, *mut T>;

    fn into_iter(self) -> Self::IntoIter {
        self.elements.iter()
    }
}

// Allow for mutable iteration
impl<'a, T> IntoIterator for &'a mut PointerVector<T> {
    type Item = &'a mut *mut T;
    type IntoIter = std::slice::IterMut<'a, *mut T>;

    fn into_iter(self) -> Self::IntoIter {
        self.elements.iter_mut()
    }
}

impl<T> Index<usize> for PointerVector<T> {
    type Output = *mut T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.elements[index]
    }
}
