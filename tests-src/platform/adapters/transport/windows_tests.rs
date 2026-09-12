//! `src/platform/adapters/transport/windows.rs` 的仅测试辅助：Authz 访问检查与 SID/描述符封装。
//! 自源文件整段移入，模块层级不变，经 #[path] 引用，不进发布包。
//! 历史消费者为 owner-only transport ACL 验收（0342f92da），该测试在分层公开 API
//! 契约重构中移除后实现保留待重新接线，暂以 dead_code 容忍未用告警。
#![allow(dead_code)]

use std::io;
use std::os::windows::io::AsRawHandle;
use std::path::Path;

use windows::Win32::Foundation::{
    ERROR_SUCCESS, HANDLE, HLOCAL, LocalFree, LUID, WIN32_ERROR,
};
use windows::Win32::Security::Authorization::{
    AUTHZ_ACCESS_CHECK_FLAGS, AUTHZ_ACCESS_REPLY, AUTHZ_ACCESS_REQUEST,
    AUTHZ_CLIENT_CONTEXT_HANDLE, AUTHZ_GENERATE_RESULTS, AUTHZ_RESOURCE_MANAGER_HANDLE,
    AUTHZ_RM_FLAG_NO_AUDIT, AUTHZ_SKIP_TOKEN_GROUPS, AuthzAccessCheck, AuthzFreeContext,
    AuthzFreeResourceManager, AuthzInitializeContextFromSid, AuthzInitializeResourceManager,
    ConvertStringSidToSidW, GetNamedSecurityInfoW, GetSecurityInfo, SE_FILE_OBJECT,
};
use windows::Win32::Security::{DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, PSID, PSECURITY_DESCRIPTOR};
use windows::Win32::Storage::FileSystem::FILE_ALL_ACCESS;
use windows::core::PCWSTR;

use super::{current_process_user_sid_string, wide_null, windows_error};

const FOREIGN_TEST_SID: &str = "S-1-5-21-111111111-222222222-333333333-1001";

#[cfg(test)]
fn win32_status(status: WIN32_ERROR) -> io::Result<()> {
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(io::Error::from_raw_os_error(status.0 as i32))
    }
}

#[cfg(test)]
struct TestAuthzContext {
    handle: AUTHZ_CLIENT_CONTEXT_HANDLE,
}

#[cfg(test)]
impl TestAuthzContext {
    fn new(sid: PSID, manager: &TestAuthzResourceManager) -> io::Result<Self> {
        let mut handle = AUTHZ_CLIENT_CONTEXT_HANDLE::default();
        unsafe {
            // SAFETY: SID 与 resource manager 在调用期间有效；跳过组解析以允许任意测试 SID。
            AuthzInitializeContextFromSid(
                AUTHZ_SKIP_TOKEN_GROUPS,
                sid,
                manager.handle,
                None,
                LUID::default(),
                None,
                &mut handle,
            )
        }
        .map_err(windows_error)?;
        Ok(Self { handle })
    }
}

#[cfg(test)]
impl Drop for TestAuthzContext {
    fn drop(&mut self) {
        unsafe {
            // SAFETY: handle 由 AuthzInitializeContextFromSid 返回且只在此释放一次。
            let _ = AuthzFreeContext(self.handle);
        }
    }
}

#[cfg(test)]
struct TestAuthzResourceManager {
    handle: AUTHZ_RESOURCE_MANAGER_HANDLE,
}

#[cfg(test)]
impl TestAuthzResourceManager {
    fn new() -> io::Result<Self> {
        let mut handle = AUTHZ_RESOURCE_MANAGER_HANDLE::default();
        unsafe {
            // SAFETY: 不提供回调；输出句柄指针有效，名称为空且关闭审计。
            AuthzInitializeResourceManager(
                AUTHZ_RM_FLAG_NO_AUDIT.0,
                None,
                None,
                None,
                PCWSTR::null(),
                &mut handle,
            )
        }
        .map_err(windows_error)?;
        Ok(Self { handle })
    }
}

#[cfg(test)]
impl Drop for TestAuthzResourceManager {
    fn drop(&mut self) {
        unsafe {
            // SAFETY: handle 由 AuthzInitializeResourceManager 返回且只在此释放一次。
            let _ = AuthzFreeResourceManager(self.handle);
        }
    }
}

#[cfg(test)]
struct TestSid {
    sid: PSID,
}

#[cfg(test)]
impl TestSid {
    fn from_string(value: &str) -> io::Result<Self> {
        let wide = wide_null(value.as_ref());
        let mut sid = PSID::default();
        unsafe {
            // SAFETY: SID 文本为存活的 NUL 结尾 UTF-16，输出指针指向已初始化存储。
            ConvertStringSidToSidW(PCWSTR(wide.as_ptr()), &mut sid)
        }
        .map_err(windows_error)?;
        if sid.is_invalid() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "SID conversion returned a null SID",
            ));
        }
        Ok(Self { sid })
    }
}

#[cfg(test)]
impl Drop for TestSid {
    fn drop(&mut self) {
        unsafe {
            // SAFETY: sid 来自 ConvertStringSidToSidW，尚未转移且只在此释放一次。
            let _ = LocalFree(Some(HLOCAL(self.sid.0)));
        }
    }
}

#[cfg(test)]
struct QueriedSecurityDescriptor {
    descriptor: PSECURITY_DESCRIPTOR,
}

#[cfg(test)]
impl QueriedSecurityDescriptor {
    fn from_path(path: &Path) -> io::Result<Self> {
        let wide = wide_null(path.as_os_str());
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        let status = unsafe {
            // SAFETY: 路径为存活的 NUL 结尾 UTF-16，输出指针指向已初始化存储。
            GetNamedSecurityInfoW(
                PCWSTR(wide.as_ptr()),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                None,
                None,
                None,
                None,
                &mut descriptor,
            )
        };
        win32_status(status)?;
        Self::from_allocated(descriptor)
    }

    fn from_handle(handle: HANDLE) -> io::Result<Self> {
        let mut descriptor = PSECURITY_DESCRIPTOR::default();
        let status = unsafe {
            // SAFETY: 调用期间句柄有效，输出指针指向已初始化存储。
            GetSecurityInfo(
                handle,
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                None,
                None,
                None,
                None,
                Some(&mut descriptor),
            )
        };
        win32_status(status)?;
        Self::from_allocated(descriptor)
    }

    fn from_allocated(descriptor: PSECURITY_DESCRIPTOR) -> io::Result<Self> {
        if descriptor.0.is_null() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "security query returned a null descriptor",
            ));
        }
        Ok(Self { descriptor })
    }
}

#[cfg(test)]
impl Drop for QueriedSecurityDescriptor {
    fn drop(&mut self) {
        unsafe {
            // SAFETY: descriptor 来自 Get*SecurityInfo，所有权尚未转移且只在此释放一次。
            let _ = LocalFree(Some(HLOCAL(self.descriptor.0)));
        }
    }
}

#[cfg(test)]
fn authz_grants_full_file_access(
    descriptor: PSECURITY_DESCRIPTOR,
    sid: PSID,
    manager: &TestAuthzResourceManager,
) -> io::Result<bool> {
    let context = TestAuthzContext::new(sid, manager)?;
    let request = AUTHZ_ACCESS_REQUEST {
        DesiredAccess: FILE_ALL_ACCESS.0,
        ..Default::default()
    };
    let mut granted = 0u32;
    let mut audit = AUTHZ_GENERATE_RESULTS::default();
    let mut error = 0u32;
    let mut reply = AUTHZ_ACCESS_REPLY {
        ResultListLength: 1,
        GrantedAccessMask: &mut granted,
        SaclEvaluationResults: &mut audit,
        Error: &mut error,
    };
    unsafe {
        // SAFETY: context、描述符与请求/响应缓冲在同步访问检查期间均存活且尺寸正确。
        AuthzAccessCheck(
            AUTHZ_ACCESS_CHECK_FLAGS(0),
            context.handle,
            &request,
            None,
            descriptor,
            None,
            &mut reply,
            None,
        )
    }
    .map_err(windows_error)?;
    Ok(error == ERROR_SUCCESS.0 && granted & FILE_ALL_ACCESS.0 == FILE_ALL_ACCESS.0)
}

#[cfg(test)]
fn descriptor_is_current_user_only(descriptor: &QueriedSecurityDescriptor) -> io::Result<bool> {
    let manager = TestAuthzResourceManager::new()?;
    let current_user = TestSid::from_string(&current_process_user_sid_string()?)?;
    let foreign = TestSid::from_string(FOREIGN_TEST_SID)?;
    Ok(
        authz_grants_full_file_access(descriptor.descriptor, current_user.sid, &manager)?
            && !authz_grants_full_file_access(descriptor.descriptor, foreign.sid, &manager)?,
    )
}

#[cfg(test)]
pub(super) fn endpoint_permissions_are_private_for_test(
    endpoint: &str,
) -> io::Result<Option<bool>> {
    let pipe = OpenOptions::new()
        .read(true)
        .write(true)
        .share_mode(0)
        .open(endpoint)?;
    let descriptor = QueriedSecurityDescriptor::from_handle(HANDLE(pipe.as_raw_handle()))?;
    Ok(Some(descriptor_is_current_user_only(&descriptor)?))
}

#[cfg(test)]
pub(super) fn discovery_permissions_are_private_for_test(path: &Path) -> io::Result<Option<bool>> {
    let descriptor = QueriedSecurityDescriptor::from_path(path)?;
    Ok(Some(descriptor_is_current_user_only(&descriptor)?))
}
