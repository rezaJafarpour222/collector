use anyhow::{Result, anyhow};
use chrono::{Datelike, Duration, NaiveDate, Timelike};

/// Produce every "YYYY/MM/DD HH:00" Jalali timestamp from start-day 00:00
/// to end-day 23:00 inclusive.
pub fn generate_hourly_timestamps(start: &str, end: &str) -> Result<Vec<String>> {
    let (sy, sm, sd) = parse_jalali(start)?;
    let (ey, em, ed) = parse_jalali(end)?;

    let (gy, gm, gd) = jalali_to_gregorian(sy, sm, sd)?;
    let (egy, egm, egd) = jalali_to_gregorian(ey, em, ed)?;

    let start_date =
        NaiveDate::from_ymd_opt(gy, gm, gd).ok_or_else(|| anyhow!("invalid start date {start}"))?;
    let end_date =
        NaiveDate::from_ymd_opt(egy, egm, egd).ok_or_else(|| anyhow!("invalid end date {end}"))?;

    let mut current = start_date.and_hms_opt(0, 0, 0).unwrap();
    let end_dt = end_date.and_hms_opt(23, 0, 0).unwrap();

    if current > end_dt {
        return Err(anyhow!("START_DATE must be <= END_DATE"));
    }

    let mut out = Vec::new();
    while current <= end_dt {
        let (jy, jm, jd) = gregorian_to_jalali(current.year(), current.month(), current.day());
        out.push(format!(
            "{:04}/{:02}/{:02} {:02}:00",
            jy,
            jm,
            jd,
            current.hour()
        ));
        current += Duration::hours(1);
    }
    Ok(out)
}

fn parse_jalali(s: &str) -> Result<(i32, u32, u32)> {
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 3 {
        return Err(anyhow!("expected YYYY/MM/DD, got '{s}'"));
    }
    let y: i32 = parts[0].parse()?;
    let m: u32 = parts[1].parse()?;
    let d: u32 = parts[2].parse()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return Err(anyhow!("invalid jalali date: {s}"));
    }
    Ok((y, m, d))
}

fn jalali_to_gregorian(jy: i32, jm: u32, jd: u32) -> Result<(i32, u32, u32)> {
    let jy = jy - 979;
    let jm = jm as i32;
    let jd = jd as i32;

    let mut days = 365 * jy
        + (jy / 33) * 8
        + ((jy % 33) + 3) / 4
        + 78
        + jd
        + if jm < 7 {
            (jm - 1) * 31
        } else {
            (jm - 7) * 30 + 186
        };

    let mut gy = 1600 + 400 * (days / 146097);
    days %= 146097;
    if days > 36524 {
        days -= 1;
        gy += 100 * (days / 36524);
        days %= 36524;
        if days >= 365 {
            days += 1;
        }
    }
    gy += 4 * (days / 1461);
    days %= 1461;
    if days > 365 {
        gy += (days - 1) / 365;
        days = (days - 1) % 365;
    }
    let gd_serial = days + 1;

    let leap = (gy % 4 == 0 && gy % 100 != 0) || (gy % 400 == 0);
    let sal_a: [i32; 13] = [
        0,
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];

    let mut gm = 1;
    let mut gd = gd_serial;
    for m in 1..=12usize {
        if gd <= sal_a[m] {
            gm = m;
            break;
        }
        gd -= sal_a[m];
    }
    Ok((gy, gm as u32, gd as u32))
}

fn gregorian_to_jalali(gy: i32, gm: u32, gd: u32) -> (i32, u32, u32) {
    const G_D_M: [i32; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];

    let (mut gy, gm, gd) = (gy, gm as i32, gd as i32);
    let mut jy;
    if gy > 1600 {
        jy = 979;
        gy -= 1600;
    } else {
        jy = 0;
        gy -= 621;
    }
    let gy2 = if gm > 2 { gy + 1 } else { gy };
    let mut days = 365 * gy + (gy2 + 3) / 4 - (gy2 + 99) / 100 + (gy2 + 399) / 400 - 80
        + gd
        + G_D_M[(gm - 1) as usize];

    jy += 33 * (days / 12053);
    days %= 12053;
    jy += 4 * (days / 1461);
    days %= 1461;
    if days > 365 {
        jy += (days - 1) / 365;
        days = (days - 1) % 365;
    }
    let jm = if days < 186 {
        1 + days / 31
    } else {
        7 + (days - 186) / 30
    } as u32;
    let jd = if days < 186 {
        1 + days % 31
    } else {
        1 + (days - 186) % 30
    } as u32;
    (jy, jm, jd)
}
