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

import shared3p;
import stdlib;

domain pd_shared3p shared3p;

void main() {
    // public scalars
    bool    b   = argument("b");
    uint8   u8  = argument("u8");
    uint16  u16 = argument("u16");
    uint32  u32 = argument("u32");
    uint64  u64 = argument("u64");
    int8    i8  = argument("i8");
    int16   i16 = argument("i16");
    int32   i32 = argument("i32");
    int64   i64 = argument("i64");
    float32 f32 = argument("f32");
    float64 f64 = argument("f64");
    string  s1  = argument("s1");
    string  s2  = argument("s2");

    // public vectors
    bool    [[1]] v_b   = argument("v_b");
    uint8   [[1]] v_u8  = argument("v_u8");
    uint16  [[1]] v_u16 = argument("v_u16");
    uint32  [[1]] v_u32 = argument("v_u32");
    uint64  [[1]] v_u64 = argument("v_u64");
    int8    [[1]] v_i8  = argument("v_i8");
    int16   [[1]] v_i16 = argument("v_i16");
    int32   [[1]] v_i32 = argument("v_i32");
    int64   [[1]] v_i64 = argument("v_i64");
    float32 [[1]] v_f32 = argument("v_f32");
    float64 [[1]] v_f64 = argument("v_f64");

    // public empty vectors
    bool    [[1]] ev_b   = argument("ev_b");
    uint8   [[1]] ev_u8  = argument("ev_u8");
    uint16  [[1]] ev_u16 = argument("ev_u16");
    uint32  [[1]] ev_u32 = argument("ev_u32");
    uint64  [[1]] ev_u64 = argument("ev_u64");
    int8    [[1]] ev_i8  = argument("ev_i8");
    int16   [[1]] ev_i16 = argument("ev_i16");
    int32   [[1]] ev_i32 = argument("ev_i32");
    int64   [[1]] ev_i64 = argument("ev_i64");
    float32 [[1]] ev_f32 = argument("ev_f32");
    float64 [[1]] ev_f64 = argument("ev_f64");

    // classified scalars
    pd_shared3p bool         c_b    = argument("c_b");
    pd_shared3p uint8        c_u8   = argument("c_u8");
    pd_shared3p uint16       c_u16  = argument("c_u16");
    pd_shared3p uint32       c_u32  = argument("c_u32");
    pd_shared3p uint64       c_u64  = argument("c_u64");
    pd_shared3p int8         c_i8   = argument("c_i8");
    pd_shared3p int16        c_i16  = argument("c_i16");
    pd_shared3p int32        c_i32  = argument("c_i32");
    pd_shared3p int64        c_i64  = argument("c_i64");
    pd_shared3p xor_uint8    c_xu8  = argument("c_xu8");
    pd_shared3p xor_uint16   c_xu16 = argument("c_xu16");
    pd_shared3p xor_uint32   c_xu32 = argument("c_xu32");
    pd_shared3p xor_uint64   c_xu64 = argument("c_xu64");
    pd_shared3p float32      c_f32  = argument("c_f32");
    pd_shared3p float64      c_f64  = argument("c_f64");
    // pd_shared3p string       c_s1   = argument("c_s1");
    // pd_shared3p string       c_s2   = argument("c_s2");

    // classified vectors
    pd_shared3p bool        [[1]] c_v_b    = argument("c_v_b");
    pd_shared3p uint8       [[1]] c_v_u8   = argument("c_v_u8");
    pd_shared3p uint16      [[1]] c_v_u16  = argument("c_v_u16");
    pd_shared3p uint32      [[1]] c_v_u32  = argument("c_v_u32");
    pd_shared3p uint64      [[1]] c_v_u64  = argument("c_v_u64");
    pd_shared3p int8        [[1]] c_v_i8   = argument("c_v_i8");
    pd_shared3p int16       [[1]] c_v_i16  = argument("c_v_i16");
    pd_shared3p int32       [[1]] c_v_i32  = argument("c_v_i32");
    pd_shared3p int64       [[1]] c_v_i64  = argument("c_v_i64");
    pd_shared3p xor_uint8   [[1]] c_v_xu8  = argument("c_v_xu8");
    pd_shared3p xor_uint16  [[1]] c_v_xu16 = argument("c_v_xu16");
    pd_shared3p xor_uint32  [[1]] c_v_xu32 = argument("c_v_xu32");
    pd_shared3p xor_uint64  [[1]] c_v_xu64 = argument("c_v_xu64");
    pd_shared3p float32     [[1]] c_v_f32  = argument("c_v_f32");
    pd_shared3p float64     [[1]] c_v_f64  = argument("c_v_f64");

    // classified empty vectors
    pd_shared3p bool        [[1]] c_ev_b    = argument("c_ev_b");
    pd_shared3p uint8       [[1]] c_ev_u8   = argument("c_ev_u8");
    pd_shared3p uint16      [[1]] c_ev_u16  = argument("c_ev_u16");
    pd_shared3p uint32      [[1]] c_ev_u32  = argument("c_ev_u32");
    pd_shared3p uint64      [[1]] c_ev_u64  = argument("c_ev_u64");
    pd_shared3p int8        [[1]] c_ev_i8   = argument("c_ev_i8");
    pd_shared3p int16       [[1]] c_ev_i16  = argument("c_ev_i16");
    pd_shared3p int32       [[1]] c_ev_i32  = argument("c_ev_i32");
    pd_shared3p int64       [[1]] c_ev_i64  = argument("c_ev_i64");
    pd_shared3p xor_uint8   [[1]] c_ev_xu8  = argument("c_ev_xu8");
    pd_shared3p xor_uint16  [[1]] c_ev_xu16 = argument("c_ev_xu16");
    pd_shared3p xor_uint32  [[1]] c_ev_xu32 = argument("c_ev_xu32");
    pd_shared3p xor_uint64  [[1]] c_ev_xu64 = argument("c_ev_xu64");
    pd_shared3p float32     [[1]] c_ev_f32  = argument("c_ev_f32");
    pd_shared3p float64     [[1]] c_ev_f64  = argument("c_ev_f64");

    // public scalars
    publish("b",      b);
    publish("u8",     u8);
    publish("u16",    u16);
    publish("u32",    u32);
    publish("u64",    u64);
    publish("i8",     i8);
    publish("i16",    i16);
    publish("i32",    i32);
    publish("i64",    i64);
    publish("f32",    f32);
    publish("f64",    f64);
    publish("s1",     s1);
    publish("s2",     s2);

    // public vectors
    publish("v_b",      v_b);
    publish("v_u8",     v_u8);
    publish("v_u16",    v_u16);
    publish("v_u32",    v_u32);
    publish("v_u64",    v_u64);
    publish("v_i8",     v_i8);
    publish("v_i16",    v_i16);
    publish("v_i32",    v_i32);
    publish("v_i64",    v_i64);
    publish("v_f32",    v_f32);
    publish("v_f64",    v_f64);

    // public empty vectors
    publish("ev_b",      ev_b);
    publish("ev_u8",     ev_u8);
    publish("ev_u16",    ev_u16);
    publish("ev_u32",    ev_u32);
    publish("ev_u64",    ev_u64);
    publish("ev_i8",     ev_i8);
    publish("ev_i16",    ev_i16);
    publish("ev_i32",    ev_i32);
    publish("ev_i64",    ev_i64);
    publish("ev_f32",    ev_f32);
    publish("ev_f64",    ev_f64);

    // classified scalars
    publish("c_b",      c_b);
    publish("c_u8",     c_u8);
    publish("c_u16",    c_u16);
    publish("c_u32",    c_u32);
    publish("c_u64",    c_u64);
    publish("c_i8",     c_i8);
    publish("c_i16",    c_i16);
    publish("c_i32",    c_i32);
    publish("c_i64",    c_i64);
    publish("c_xu8",    c_xu8);
    publish("c_xu16",   c_xu16);
    publish("c_xu32",   c_xu32);
    publish("c_xu64",   c_xu64);
    publish("c_f32",    c_f32);
    publish("c_f64",    c_f64);
    // publish("c_s1",     c_s1);
    // publish("c_s2",     c_s2);

    // classified vectors
    publish("c_v_b",      c_v_b);
    publish("c_v_u8",     c_v_u8);
    publish("c_v_u16",    c_v_u16);
    publish("c_v_u32",    c_v_u32);
    publish("c_v_u64",    c_v_u64);
    publish("c_v_i8",     c_v_i8);
    publish("c_v_i16",    c_v_i16);
    publish("c_v_i32",    c_v_i32);
    publish("c_v_i64",    c_v_i64);
    publish("c_v_xu8",    c_v_xu8);
    publish("c_v_xu16",   c_v_xu16);
    publish("c_v_xu32",   c_v_xu32);
    publish("c_v_xu64",   c_v_xu64);
    publish("c_v_f32",    c_v_f32);
    publish("c_v_f64",    c_v_f64);

    // classified empty vectors
    publish("c_ev_b",      c_ev_b);
    publish("c_ev_u8",     c_ev_u8);
    publish("c_ev_u16",    c_ev_u16);
    publish("c_ev_u32",    c_ev_u32);
    publish("c_ev_u64",    c_ev_u64);
    publish("c_ev_i8",     c_ev_i8);
    publish("c_ev_i16",    c_ev_i16);
    publish("c_ev_i32",    c_ev_i32);
    publish("c_ev_i64",    c_ev_i64);
    publish("c_ev_xu8",    c_ev_xu8);
    publish("c_ev_xu16",   c_ev_xu16);
    publish("c_ev_xu32",   c_ev_xu32);
    publish("c_ev_xu64",   c_ev_xu64);
    publish("c_ev_f32",    c_ev_f32);
    publish("c_ev_f64",    c_ev_f64);
}
