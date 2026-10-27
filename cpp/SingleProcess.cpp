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

/*
 * This code is a convenience wrapper for spawning a VM executing a single program
 * for use with Protocol Core.
 */

#ifndef SHAREMIND_ICONST
#define SHAREMIND_ICONST
#endif /* SHAREMIND_ICONST */


#include <cstdio>
#include "sharemind/libvm/Process.h"
#include "sharemind/libvm/Program.h"
#include "sharemind/libvm/Vm.h"
#include <sharemind/AssertReturn.h>
#include <sharemind/libmodapi/libmodapi.h>
#include <sharemind/module-apis/api_0x1.h>
#include "singleprocess.h"
#include <array>
#include <vector>
#include <memory>
#include <cstddef>

namespace sharemind {
namespace {

#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wunused-function"
#ifdef __clang__
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wunused-member-function"
#endif
#define DEFINE_EXCEPTION(name, msg) \
    SHAREMIND_DECLARE_EXCEPTION_CONST_MSG_NOINLINE(sharemind::Exception, name);\
    SHAREMIND_DEFINE_EXCEPTION_CONST_MSG_NOINLINE(sharemind::Exception,, \
                                                  name, \
                                                  msg)
DEFINE_EXCEPTION(ModuleImplementationLimitsReachedException,
                 "Module implementation limits reached!")
DEFINE_EXCEPTION(ModuleErrorException, "Programming fault in the module!")
DEFINE_EXCEPTION(ModuleGeneralErrorException,
                 "General runtime error in the module!")
DEFINE_EXCEPTION(ModuleInvalidCallException,
                 "The system call was called improperly by the bytecode!")
DEFINE_EXCEPTION(ModuleMissingFacilityException,
                 "A required facility was not provided by Sharemind!")
DEFINE_EXCEPTION(ModuleInvalidPdConfiguration,
                 "The protection domain configuration given was invalid or "
                 "erroneous!")
DEFINE_EXCEPTION(ModuleInvalidModuleConfiguration,
                 "The module configuration given was invalid or erroneous!")
DEFINE_EXCEPTION(ModuleAccessDeniedException, "Access denied by module!")
#ifdef __clang__
#pragma clang diagnostic pop
#endif
#pragma GCC diagnostic pop


void printException(std::exception const & e, int level =  0) {
    printf("[%d] exception: %s\n", level, e.what());
    try {
        std::rethrow_if_nested(e);
    } catch (std::exception const & nestedException) {
        printException(nestedException, level + 1);
    } catch (...) {}
}

void printCurrentException(char const * const msg) {
    printf("%s", msg);
    auto ex = std::current_exception();
    try {
        if (ex) {
            std::rethrow_exception(ex);
        }
    } catch (std::exception const & e) {
        printException(e);
    }
};

} // namespace anonymous

class SyscallWrapperImpl;

extern "C"
ProcessExitStatus spawn(
    char const * filename,
    FinderMut const * facilityfinder,
    FinderConst const * syscallfinder)
try {

    Vm vm;
    // Rust Prereq: function that can acquire module syscalls by name
    // in the application server, all loaded module syscalls are cached
    vm.setSyscallFinder([syscallfinder](std::string const & name) {
        auto ptr = static_cast<SharemindSyscallWrapper const *>(syscallfinder->find(syscallfinder, name.c_str()));

        // NOT GOOD, new shared_ptr created on each (reoccurring) syscall fetch
        if (ptr) {
            return std::make_shared<SyscallWrapperImpl>(*ptr, name.c_str());
        } else {
            return std::shared_ptr<SyscallWrapperImpl>();
        }
    });

    Program program(vm, filename);
    // Rust Prereq: function that can acquire facilities by name
    // in the application server, the following facilities are given:
    // * AccessControlProcessFacility (probably for datastore accesses)
    // * ProcessFacility (yes, ProcessFacility is one of many other process<space>facilities)
    // * DataStoreFactory (dont know what this does)
    // * Profiler

    // In addition to process facilities, the pdpi created by the process
    // needs facilities, in the application server example, given are
    // * ProcessFacility
    // * AccessControlProcessFacility
    // * RandomFacility
    // We might be able to use the hierarchical nature of fmodapi facility finders to avoid
    // giving the created pdpi its own facilities (since we only anticipate one single pdpi).


    Process process(program);

    process.setFacilityFinder(
        [facilityfinder](char const * name){
            return std::shared_ptr<void>(
                std::shared_ptr<void>(),
                facilityfinder->find(facilityfinder, name)
            );
        });


    try {
        process.run();
    } catch (Process::TrapException const &) {
        printCurrentException("Process exited with trap: ");
        return ProcessExitStatus::PROCESS_EXIT_TRAP_ERROR;
    } catch (Process::SystemCallErrorException const &) {
        printCurrentException("Process exited with exception from system call: ");
        return ProcessExitStatus::PROCESS_EXIT_SYSCALL_ERROR;
    } catch (Process::RuntimeException const &) {
        printCurrentException("Process exited with runtime exception: ");
        return ProcessExitStatus::PROCESS_EXIT_RUNTIME_ERROR;
    } catch (...) {
        printCurrentException("Process exited with exception: ");
        return ProcessExitStatus::PROCESS_EXIT_UNKNOWN_ERROR;
    }

    return ProcessExitStatus::PROCESS_EXIT_OK;
} catch (std::exception const &) {
    printCurrentException("Error spawning the process: ");
    return ProcessExitStatus::PROCESS_EXIT_SPAWN_ERROR;
}

/** \brief Wraps a C++-style (libvm) system call context for exposition to
           C-style (libmodapi) system calls. */
class OldSyscallContext final: public SharemindModuleApi0x1SyscallContext {

public: /* Types: */

    using PublicMemoryPointer = Vm::SyscallContext::PublicMemoryPointer;

public: /* Methods: */

    OldSyscallContext(Vm::SyscallContext & context, void * moduleHandle_)
        : SharemindModuleApi0x1SyscallContext{
            &context,
            nullptr, // process_internal
            moduleHandle_,
            &get_pdpi_info,
            &processFacility,
            &publicAlloc,
            &publicFree,
            &publicMemPtrSize,
            &publicMemPtrData,
            &allocPrivate,
            &freePrivate,
            &reservePrivate,
            &releasePrivate}
    {}

    static SharemindModuleApi0x1PdpiInfo const * get_pdpi_info(
        SharemindModuleApi0x1SyscallContext * c,
        std::uint64_t pd_index)
    {
        if (auto r = fromC(c).processFacility("RustProcess")) {
            auto facility = static_cast<RustProcess*>(r.get());
            return facility->get_pdpi_info(facility, pd_index);
        }
        return nullptr;
    }

    static void * processFacility(
            SharemindModuleApi0x1SyscallContext const * c,
            char const * facilityName)
    {
        if (auto r = fromC(c).processFacility(facilityName))
            return r.get();
        return nullptr;
    }

    static std::uint64_t publicAlloc(SharemindModuleApi0x1SyscallContext * c,
                                     std::uint64_t nBytes)
    { return fromC(c).publicAlloc(nBytes).ptr; }

    static bool publicFree(SharemindModuleApi0x1SyscallContext * c,
                           std::uint64_t ptr)
    { return fromC(c).publicFree(PublicMemoryPointer{ptr}); }

    static std::size_t publicMemPtrSize(SharemindModuleApi0x1SyscallContext * c,
                                        std::uint64_t ptr)
    { return fromC(c).publicMemPtrSize(PublicMemoryPointer{ptr}); }

    static void * publicMemPtrData(SharemindModuleApi0x1SyscallContext * c,
                                   std::uint64_t ptr)
    { return fromC(c).publicMemPtrData(PublicMemoryPointer{ptr}); }

    /* Access to dynamic memory not exposed to VM instructions: */
    static void * allocPrivate(SharemindModuleApi0x1SyscallContext *,
                               std::size_t)
    { return nullptr; }

    static void freePrivate(SharemindModuleApi0x1SyscallContext *, void *) {}

    static bool reservePrivate(SharemindModuleApi0x1SyscallContext *,
                               std::size_t)
    { return false; }

    static bool releasePrivate(SharemindModuleApi0x1SyscallContext *,
                               std::size_t)
    { return false; }

private: /* Methods: */

    static Vm::SyscallContext & fromC(
            SharemindModuleApi0x1SyscallContext * const c) noexcept
    { return *static_cast<Vm::SyscallContext *>(assertReturn(assertReturn(c)->vm_internal)); }

    static Vm::SyscallContext const & fromC(
            SharemindModuleApi0x1SyscallContext const * const c) noexcept
    { return *static_cast<Vm::SyscallContext const *>(assertReturn(assertReturn(c)->vm_internal)); }

};

class SyscallWrapperImpl: public Vm::SyscallWrapper {

public:
    SyscallWrapperImpl(SharemindSyscallWrapper wrapper, const char *name)
        : m_wrapper(std::move(wrapper)), m_name(name)
    {
    }

    void operator()(
        std::vector<::SharemindCodeBlock> &arguments,
        std::vector<Vm::Reference> &references,
        std::vector<Vm::ConstReference> &creferences,
        ::SharemindCodeBlock *returnValue,
        Vm::SyscallContext &context) const final override
    {
        OldSyscallContext ctx(context, m_wrapper.internal);
        // printf("called %s\n", m_name.c_str());

        std::vector<::SharemindModuleApi0x1Reference> refonHeap(1);
        ::SharemindModuleApi0x1Reference *refs;
        if (!references.empty())
        {
            refonHeap.resize(references.size() + 1);
            refs = refonHeap.data();
            auto ptr = refs;
            for (auto const &ref : references)
            {
              ptr->pData = ref.data.get();
              ptr->size = ref.size;
              ++ptr;
            }
            ptr->pData = nullptr;
            ptr->size = 0u;
        }
        else
        {
            refs = nullptr;
        }

        std::vector<::SharemindModuleApi0x1CReference> crefonHeap(1);
        ::SharemindModuleApi0x1CReference *crefs;
        if (!creferences.empty())
        {
            crefonHeap.resize(creferences.size() + 1);
            crefs = crefonHeap.data();
            auto ptr = crefs;
            for (auto const &cref : creferences)
            {
              ptr->pData = cref.data.get();
              ptr->size = cref.size;
              ++ptr;
            }
            ptr->pData = nullptr;
            ptr->size = 0u;
        }
        else
        {
            crefs = nullptr;
        }

        auto const r = m_wrapper.callable(arguments.data(),
                                          arguments.size(),
                                          refs,
                                          crefs,
                                          returnValue,
                                          &ctx);

        switch (r)
        {
        case SHAREMIND_MODULE_API_0x1_OK:
            return;
        case SHAREMIND_MODULE_API_0x1_OUT_OF_MEMORY:
            throw std::bad_alloc();
        case SHAREMIND_MODULE_API_0x1_IMPLEMENTATION_LIMITS_REACHED:
            throw ModuleImplementationLimitsReachedException();
        case SHAREMIND_MODULE_API_0x1_MODULE_ERROR:
            throw ModuleErrorException();
        case SHAREMIND_MODULE_API_0x1_GENERAL_ERROR:
            throw ModuleGeneralErrorException();
        case SHAREMIND_MODULE_API_0x1_INVALID_CALL:
            throw ModuleInvalidCallException();
        case SHAREMIND_MODULE_API_0x1_MISSING_FACILITY:
            throw ModuleMissingFacilityException();
        case SHAREMIND_MODULE_API_0x1_INVALID_PD_CONFIGURATION:
            throw ModuleInvalidPdConfiguration();
        case SHAREMIND_MODULE_API_0x1_INVALID_MODULE_CONFIGURATION:
            throw ModuleInvalidModuleConfiguration();
        case SHAREMIND_MODULE_API_0x1_ACCESS_DENIED:
            throw ModuleAccessDeniedException();
        }
    }

private:
    SharemindSyscallWrapper m_wrapper;
    std::string m_name;
};

} /* namespace sharemind { */
