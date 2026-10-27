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

use std::sync::mpsc::{channel, Receiver, Sender};

pub fn create_contexts() -> [WorkerContext; 3] {
    // Cant use arrays or vectors comfortably as
    // moving the channels out by index is not
    // able to be checked by the compiler.
    let channels = (
        channel(),
        channel(),
        channel(),
        channel(),
        channel(),
        channel(),
    );
    [
        WorkerContext {
            id: 1,
            send_left: channels.5 .0,
            recv_left: channels.0 .1,
            send_right: channels.2 .0,
            recv_right: channels.1 .1,
        },
        WorkerContext {
            id: 2,
            send_left: channels.1 .0,
            recv_left: channels.2 .1,
            send_right: channels.4 .0,
            recv_right: channels.3 .1,
        },
        WorkerContext {
            id: 3,
            send_left: channels.3 .0,
            recv_left: channels.4 .1,
            send_right: channels.0 .0,
            recv_right: channels.5 .1,
        },
    ]
}

#[derive(Debug)]
pub struct WorkerContext {
    pub id: usize,
    pub send_left: Sender<Vec<u8>>,
    pub send_right: Sender<Vec<u8>>,
    pub recv_left: Receiver<Vec<u8>>,
    pub recv_right: Receiver<Vec<u8>>,
}
