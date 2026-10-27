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

#![cfg(feature = "ctrl")]

use sharemind_runtime::ffi::{
    ctrl::cmodapi::pd::Value,
    loghard::LogHardLogger,
    primitive_types::{VmData, VmDataRef, VmDataView, VmType},
};
use sharemind_runtime::helper::client::ProtectionDomainService;
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

#[test]
fn cmodapi_test() {
    // Test configuration
    let module_path = Path::new("libsharemind_mod_shared3p_ctrl.so");
    let pdk_name = "shared3p";
    let pd_name = "pd_shared3p";
    let pd_conf = None;
    let node_count = 3;

    // Create PD
    let logger = LogHardLogger::default();
    let pd = ProtectionDomainService::new(&logger, module_path, pdk_name, pd_name, pd_conf, None)
        .unwrap();

    // Create test data
    const ELEMENT_COUNT: usize = 2;
    let test_datas = vec![
        VmData::random(VmType::Bool, ELEMENT_COUNT),
        VmData::random(VmType::Int8, ELEMENT_COUNT),
        VmData::random(VmType::Int16, ELEMENT_COUNT),
        VmData::random(VmType::Int32, ELEMENT_COUNT),
        VmData::random(VmType::Int64, ELEMENT_COUNT),
        VmData::random(VmType::Uint8, ELEMENT_COUNT),
        VmData::random(VmType::Uint16, ELEMENT_COUNT),
        VmData::random(VmType::Uint32, ELEMENT_COUNT),
        VmData::random(VmType::Uint64, ELEMENT_COUNT),
        VmData::random(VmType::Float32, ELEMENT_COUNT),
        VmData::random(VmType::Float64, ELEMENT_COUNT),
    ];

    let plaintext_values = test_datas
        .iter()
        .enumerate()
        .map(|(i, data)| {
            let value = Value {
                pd_name: pd_name.into(),
                type_name: data.vm_type().type_name().to_owned(),
                data: data.as_bytes().to_owned(),
            };
            (format!("val_{i}"), value)
        })
        .collect::<HashMap<_, _>>();
    assert_eq!(test_datas.len(), plaintext_values.len());

    // Classify
    let classified_value_sets = pd.classify(&plaintext_values).unwrap();

    assert_eq!(
        classified_value_sets.len(),
        node_count,
        "Incorrect number of classified value sets"
    );
    assert!(
        classified_value_sets
            .iter()
            .all(|vset| vset.len() == plaintext_values.len()),
        "Classified value sets have incorrect number of values"
    );
    assert!(
        classified_value_sets.iter().all(|vset| vset
            .keys()
            .collect::<HashSet<_>>()
            .eq(&plaintext_values.keys().collect::<HashSet<_>>())),
        "Classified value sets have incorrect sets of keys"
    );

    // Declassify
    let declassified_values = pd.declassify(&classified_value_sets).unwrap();

    // Validate result
    assert_eq!(plaintext_values, declassified_values);

    // Validate with the original data too (for fun)
    for (i, test_data) in test_datas.iter().enumerate() {
        let dvalue = declassified_values.get(&format!("val_{i}")).unwrap();
        let vm_type = VmType::from_name(&dvalue.type_name).unwrap();
        let vm_data_view = VmDataView::new(vm_type, &dvalue.data).unwrap();
        assert_eq!(*test_data, vm_data_view, "Incorrect declassified value");
    }
}
