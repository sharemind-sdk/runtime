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
    print("Running Main Algorithm (shared3p)...");
    pd_shared3p uint64[[1]] in1  = argument("in1");
    print("Multiplying two vectors:");
    printVector(declassify(in1));
    print("and");
    pd_shared3p uint64[[1]] in2  = argument("in2");
    printVector(declassify(in2));
    pd_shared3p uint64[[1]] out1 = in1 * in2;
    print("Result is:");
    printVector(declassify(out1));
    publish("out1", out1);
}
