#
# Copyright (C) 2026 Cybernetica
#
# Research/Commercial License Usage
# Licensees holding a valid Research License or Commercial License
# for the Software may use this file according to the written
# agreement between you and Cybernetica.
#
# GNU General Public License Usage
# Alternatively, this file may be used under the terms of the GNU
# General Public License version 3.0 as published by the Free Software
# Foundation and appearing in the file LICENSE included in the
# packaging of this file.  Please review the following information to
# ensure the GNU General Public License version 3.0 requirements will be
# met: http://www.gnu.org/copyleft/gpl-3.0.html.
#
# For further information, please contact us at sharemind@cyber.ee.
#

cmake_minimum_required(VERSION 3.9)
set(CMAKE_CXX_STANDARD 17)
project(rust-runtime-supplementary-lib)

find_package(SharemindLibRandom REQUIRED)
find_package(SharemindLibDataStoreManager REQUIRED)
find_package(SharemindLibVm REQUIRED)
if(ENABLE_PROFILER)
    find_package(SharemindLibExecutionProfiler REQUIRED)
endif()
if(ENABLE_TABLEDB)
    find_package(SharemindModTableDb 0.5.0 REQUIRED)
endif()

add_library(supplementary STATIC
    "DataStoreManager.cpp"
    "libdatastoremanager.h"
    "SingleProcess.cpp"
    "singleprocess.h"
    "librandom.h"
    "Random.cpp"
    "ShareVectorAccess.cpp"
)

target_compile_definitions(supplementary
    PRIVATE
        "SHAREMIND_INTERNAL_"   # for including DataStoreFactory.h
    )

target_link_libraries(supplementary
    Sharemind::LibDataStoreManager
    Sharemind::LibVm
    Sharemind::LibRandom
)

if(ENABLE_PROFILER)
    message(STATUS "Profiler feature enabled")
    target_compile_definitions(supplementary PRIVATE "ENABLE_PROFILER")

    target_sources(supplementary PRIVATE
        "ExecutionProfiler.cpp"
        "libexecutionprofiler.h"
    )

    target_link_libraries(supplementary
        Sharemind::LibExecutionProfiler
    )
endif()

if(ENABLE_TABLEDB)
    message(STATUS "TableDB feature enabled")
    target_compile_definitions(supplementary PRIVATE "ENABLE_TABLEDB")

    target_sources(supplementary PRIVATE
        "TdbTypesUtil2.cpp"
        "TdbTypesUtil2.h"
    )

    target_link_libraries(supplementary
        Sharemind::ModTableDb
    )
endif()

install(TARGETS supplementary DESTINATION .)
