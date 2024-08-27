use std::{
    ffi::{c_char, c_uint},
    mem::size_of_val,
    thread::sleep,
    time::{Duration, Instant},
};

use libloading::{Library, Symbol};

fn get_lib() -> Result<Library, libloading::Error> {
    unsafe { Library::new("E:\\Experimental\\callCDll\\libs\\Sadp.dll") }
}

fn get_version_fn(
    lib: &Library,
) -> Result<Symbol<unsafe extern "C" fn() -> u32>, libloading::Error> {
    unsafe {
        let fun: Symbol<unsafe extern "C" fn() -> u32> = lib.get(b"SADP_GetSadpVersion")?;
        return Ok(fun.clone());
    }
}

fn mnt_to_string(bytes: &[i8]) -> String {
    unsafe { std::str::from_utf8_unchecked(std::mem::transmute(bytes)) }.to_string()
}

fn bytes_trim(bytes: &[i8]) -> Vec<i8> {
    let mut result: Vec<i8> = vec![];
    for &byte in bytes {
        if byte != 0 {
            result.push(byte);
        }
    }
    result
}

fn bytes_to_string(bytes: &[i8]) -> String {
    mnt_to_string(&bytes_trim(bytes))
}

fn start_fn(lib: &Library) -> Result<u32, libloading::Error> {
    unsafe {
        let cb: DeviceFindCallbackV40 = |info| {
            println!("进入");

            // println!("授权:{:?}", info.byLicensed);
            // println!("模式:{:?}", info.bySystemMode);
            // println!("版本:{:?}", bytes_to_string(&info.szEhmoeVersion));
            // println!("SDK状态:{:?}", info.bySDKServerStatus);

            println!("系列:{:?}", bytes_to_string(&info.device_info.series));
            println!("序列号:{:?}", bytes_to_string(&info.device_info.serial_no));
            println!("mac:{:?}", bytes_to_string(&info.device_info.mac));
            println!("ip:{:?}", bytes_to_string(&info.device_info.ipv4_address));
            println!(
                "子网掩码:{:?}",
                bytes_to_string(&info.device_info.ipv4_subnet_mask)
            );
            println!("端口:{:?}", info.device_info.port);
            // println!("端口:{:?}", info.device_info.dwNumberOfEncoders);
        };
        println!("长度:{}", size_of_val(&cb));
        let fun: Symbol<unsafe fn(a: DeviceFindCallbackV40, b: i32) -> u32> =
            lib.get(b"SADP_Start_V40")?;
        return Ok(fun(cb, 1));
    }
}

#[repr(C)]
struct DeviceInfo {
    device_info: SADPDeviceInfo,
    // byLicensed: c_uchar,
    // bySystemMode: c_uchar,
    // byControllerType: c_uchar,
    // szEhmoeVersion: [c_char; 16],
    // bySpecificDeviceType: c_uchar,
    // dwSDKOverTLSPort: c_uint,
    // bySecurityMode: c_uchar,
    // bySDKServerStatus: c_uchar,
    // bySDKOverTLSServerStatus: c_uchar,
    // szUserName: [c_char; 32 + 1],
    // szWifiMAC: [c_char; 20],
    // byDataFromMulticast: c_uchar,
    // bySupportEzvizUnbind: c_uchar,
    // bySupportCodeEncrypt: c_uchar,
    // byRes: [c_uchar; 429],
}

#[repr(C)]
struct SADPDeviceInfo {
    series: [c_char; 12],
    serial_no: [c_char; 48],
    mac: [c_char; 20],
    ipv4_address: [c_char; 16],
    ipv4_subnet_mask: [c_char; 16],
    device_type: c_uint,
    port: c_uint,
    // dwNumberOfEncoders: c_uint,
    // dwNumberOfHardDisk: c_uint,
    // szDeviceSoftwareVersion: [c_char; 48],
    // szDSPVersion: [c_char; 48],
    // szBootTime: [c_char; 48],
    // iResult: c_int,
    // szDevDesc: [c_char; 24],
    // szOEMinfo: [c_char; 24],
    // szIPv4Gateway: [c_char; 16],
    // szIPv6Address: [c_char; 46],
    // szIPv6Gateway: [c_char; 46],
    // byIPv6MaskLen: c_uchar,
    // bySupport: c_uchar,
    // byDhcpEnabled: c_uchar,
    // byDeviceAbility: c_uchar,
    // wHttpPort: c_ushort,
    // wDigitalChannelNum: c_ushort,
    // szCmsIPv4: [c_uchar; 16],
    // wCmsPort: c_ushort,
    // byOEMCode: c_uchar,
    // byActivated: c_uchar,
    // szBaseDesc: [c_char; 24],
    // bySupport1: c_uchar,
    // byHCPlatform: c_uchar,
    // byEnableHCPlatform: c_uchar,
    // byEZVIZCode: c_uchar,
    // dwDetailOEMCode: c_uint,
    // byModifyVerificationCode: c_uchar,
    // byMaxBindNum: c_uchar,
    // wOEMCommandPort: c_ushort,
    // bySupportWifiRegion: c_uchar,
    // byEnableWifiEnhancement: c_uchar,
    // byWifiRegion: c_uchar,
    // bySupport2: c_uchar,
}

type DeviceFindCallbackV40 = fn(info: DeviceInfo);

pub fn call_dll() {
    println!("123");
    let start = Instant::now();
    let lib = get_lib().unwrap();
    let elapsed = start.elapsed();
    println!("get lib 耗时：{:?}", elapsed);

    let start = Instant::now();
    let get_version = get_version_fn(&lib).unwrap();
    let a = unsafe { get_version() };
    let elapsed = start.elapsed();
    println!("get version 耗时：{:?}", elapsed);
    print!("{:?}", a);

    let start = Instant::now();
    let result = start_fn(&lib).unwrap();
    let elapsed = start.elapsed();
    println!("start 耗时：{:?}", elapsed);
    println!(
        "启动 SADP 服务 {}",
        if result == 1 { "成功" } else { "失败" }
    );
    sleep(Duration::from_secs(600));
    println!("退出");
}
