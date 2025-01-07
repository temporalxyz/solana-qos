pub mod ipc_parameters;
pub mod packet_bytes;
pub mod remaining_meta;
pub mod shared_stats;
pub mod xxhash;

pub fn checked_drop_privileges() -> Result<(), String> {
    // Retrieve the `SUDO_USER` environment variable
    let sudo_user = std::env::var("SUDO_USER")
        .map_err(|_| "SUDO_USER environment variable is not set. Are you running with sudo?".to_string())?;

    // Get the UID and GID of the `SUDO_USER`
    let c_user = std::ffi::CString::new(sudo_user).map_err(|_| {
        "Failed to convert username to CString".to_string()
    })?;
    let passwd = unsafe { libc::getpwnam(c_user.as_ptr()) };
    if passwd.is_null() {
        return Err("Failed to find user info for the original user"
            .to_string());
    }

    // Safety: Dereference the pointer returned by `getpwnam`
    let uid = unsafe { (*passwd).pw_uid };
    let gid = unsafe { (*passwd).pw_gid };

    // Drop privileges by setting GID and UID
    if unsafe { libc::setgid(gid) } != 0 {
        return Err(format!("Failed to set GID to {}", gid));
    }
    if unsafe { libc::setuid(uid) } != 0 {
        return Err(format!("Failed to set UID to {}", uid));
    }

    println!(
        "Privileges successfully dropped to user with UID: {}, GID: {}",
        uid, gid
    );
    Ok(())
}
