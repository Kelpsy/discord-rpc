use core_foundation::{
    base::{OSStatus, ToVoid},
    bundle::CFBundle,
    string::{CFString, CFStringRef},
    url::CFURLRef,
};

#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    fn LSSetDefaultHandlerForURLScheme(url: CFStringRef, bundle_id: CFStringRef) -> OSStatus;
    fn LSRegisterURL(bundle_url: CFURLRef, update: bool) -> OSStatus;
}

pub fn register_url(app_id: &str) -> Result<(), String> {
    let url = CFString::new(&format!("discord-{}", app_id));
    let main_bundle = CFBundle::main_bundle();
    let bundle_id = main_bundle
        .info_dictionary()
        .get(&CFString::from_static_string("CFBundleIdentifier"))
        .downcast::<CFString>()
        .ok_or_else(|| "Could not determine bundle identifier".to_string())?;
    let bundle_url = main_bundle
        .bundle_url()
        .ok_or_else(|| "Could not determine bundle URL".to_string())?;
    unsafe {
        let status = LSSetDefaultHandlerForURLScheme(
            url.to_void() as *const _,
            bundle_id.to_void() as *const _,
        );
        if status != 0 {
            return Err(format!(
                "Error in LSSetDefaultHandlerForURLScheme: {}",
                status
            ));
        }
        let status = LSRegisterURL(bundle_url.to_void() as *const _, true);
        if status != 0 {
            return Err(format!("Error in LSRegisterURL: {}", status));
        }
    }
    Ok(())
}
