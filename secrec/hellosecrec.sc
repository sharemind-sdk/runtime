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

template<domain D : shared3p, type T>
D T[[1]] square_and_multiply(D T[[1]] x, int n) {
    if (n < 0) {
        return square_and_multiply(1 / x, -n);
    } else if (n == 0) {
	D T[[1]] rv(size(x)) = 1;
        return rv;
    } else if (n == 1) {
        return x;
    } else if (n % 2 == 0) {
        return square_and_multiply(x * x, n / 2);
    } else {
        return x * square_and_multiply(x * x, (n - 1) / 2);
    }
}

void uint64_test(uint s) {
    print("Running test for type: uint64");
    pd_shared3p uint64[[1]] input(s);

    for (uint i = 0; i < s; ++i)
        input[i] = i + 1;

    print("Inputs:");
    printVector(declassify(input[0:min(s, 10::uint)]));

    pd_shared3p uint64[[1]] output = square_and_multiply(input, 3);
    print("Outputs:");
    printVector(declassify(output[0:min(s, 10::uint)]));
}

void float64_test(uint s) {
    print("Running test for type: float64");
    pd_shared3p float64[[1]] input(s);

    for (uint i = 0; i < s; ++i)
        input[i] = (float64)(i + 1) / 2;

    print("Inputs:");
    printVector(declassify(input[0:min(s, 10::uint)]));

    pd_shared3p float64[[1]] output = square_and_multiply(input, 3);
    print("Outputs:");
    printVector(declassify(output[0:min(s, 10::uint)]));
}

void main() {
    uint s = 100;

    print("Vector size: ", s);

    uint64_test(s);
    float64_test(s);
}
