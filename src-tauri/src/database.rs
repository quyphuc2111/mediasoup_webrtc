use rusqlite::{Connection, Result as SqlResult, params};
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use bcrypt::{hash, verify, DEFAULT_COST};

// ============== Device Models (existing) ==============
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SavedDevice {
    pub id: Option<i64>,
    pub ip: String,
    pub name: String,
    pub port: u16,
    pub last_used: u64,
}

// ============== User & Auth Models ==============
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub enum UserRole {
    Administrator,
    Teacher,
    Student,
}

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Administrator => "Administrator",
            UserRole::Teacher => "Teacher",
            UserRole::Student => "Student",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Administrator" => Some(UserRole::Administrator),
            "Teacher" => Some(UserRole::Teacher),
            "Student" => Some(UserRole::Student),
            _ => None,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct UserAccount {
    pub user_id: i64,
    pub user_name: String,
    pub role: String,
    pub status: bool,
    pub created_at: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LoginResponse {
    pub success: bool,
    pub message: String,
    pub user: Option<UserAccount>,
}

// ============== School Management Models ==============
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SchoolYear {
    pub school_year_id: i64,
    pub school_year_name: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Grade {
    pub grade_id: i64,
    pub grade_name: String,
    pub school_year_id: i64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Subject {
    pub subject_id: i64,
    pub subject_name: String,
    pub school_year_id: i64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PracticeTimeSlot {
    pub practice_time_slot_id: i64,
    pub practice_time_slot_name: String,
    pub school_year_id: i64,
    pub start_time: String,
    pub end_time: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RoomComputer {
    pub room_computer_id: i64,
    pub computer_name: String,
    pub ip_address: String,
    pub status: String, // 'Active', 'Repairing', 'Broken'
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Teacher {
    pub teacher_id: i64,
    pub teacher_code: String,
    pub teacher_name: String,
    pub user_id: i64,
    pub created_at: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Student {
    pub student_id: i64,
    pub student_code: String,
    pub student_name: String,
    pub user_id: i64,
    pub grade_id: i64,
    pub created_at: String,
}

// ============== Database Path ==============
pub fn get_db_path(_app: &AppHandle) -> PathBuf {
    let app_data = dirs::data_local_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join(".smartlab")))
        .or_else(|| std::env::current_dir().ok().map(|d| d.join("app_data")))
        .expect("Failed to get app data directory");
    
    std::fs::create_dir_all(&app_data).ok();
    
    app_data.join("smartlab.db")
}

// ============== Database Initialization ==============
pub fn init_database(app: &AppHandle) -> SqlResult<Connection> {
    let db_path = get_db_path(app);
    
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    
    let conn = Connection::open(&db_path)?;
    
    // Enable foreign keys
    conn.execute("PRAGMA foreign_keys = ON", [])?;
    
    // Create all tables
    create_tables(&conn)?;
    
    // Seed default data if needed
    seed_default_data(&conn)?;
    
    Ok(conn)
}

fn create_tables(conn: &Connection) -> SqlResult<()> {
    // Devices table (existing)
    conn.execute(
        "CREATE TABLE IF NOT EXISTS devices (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ip TEXT NOT NULL,
            name TEXT NOT NULL,
            port INTEGER NOT NULL,
            last_used INTEGER NOT NULL,
            UNIQUE(ip, port)
        )",
        [],
    )?;

    // UserAccounts table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS user_accounts (
            user_id INTEGER PRIMARY KEY AUTOINCREMENT,
            user_name TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            role TEXT NOT NULL CHECK(role IN ('Administrator', 'Teacher', 'Student')),
            status INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;

    // SchoolYears table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS school_years (
            school_year_id INTEGER PRIMARY KEY AUTOINCREMENT,
            school_year_name TEXT NOT NULL UNIQUE,
            start_date TEXT,
            end_date TEXT
        )",
        [],
    )?;

    // Grades table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS grades (
            grade_id INTEGER PRIMARY KEY AUTOINCREMENT,
            grade_name TEXT NOT NULL,
            school_year_id INTEGER NOT NULL,
            FOREIGN KEY (school_year_id) REFERENCES school_years(school_year_id)
        )",
        [],
    )?;

    // Subjects table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS subjects (
            subject_id INTEGER PRIMARY KEY AUTOINCREMENT,
            subject_name TEXT NOT NULL,
            school_year_id INTEGER NOT NULL,
            FOREIGN KEY (school_year_id) REFERENCES school_years(school_year_id)
        )",
        [],
    )?;

    // PracticeTimeSlots table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS practice_time_slots (
            practice_time_slot_id INTEGER PRIMARY KEY AUTOINCREMENT,
            practice_time_slot_name TEXT NOT NULL,
            school_year_id INTEGER NOT NULL,
            start_time TEXT NOT NULL,
            end_time TEXT NOT NULL,
            FOREIGN KEY (school_year_id) REFERENCES school_years(school_year_id)
        )",
        [],
    )?;

    // Teachers table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS teachers (
            teacher_id INTEGER PRIMARY KEY AUTOINCREMENT,
            teacher_code TEXT NOT NULL,
            teacher_name TEXT NOT NULL,
            user_id INTEGER NOT NULL UNIQUE,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (user_id) REFERENCES user_accounts(user_id)
        )",
        [],
    )?;

    // Students table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS students (
            student_id INTEGER PRIMARY KEY AUTOINCREMENT,
            student_code TEXT NOT NULL,
            student_name TEXT NOT NULL,
            user_id INTEGER NOT NULL UNIQUE,
            grade_id INTEGER,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (user_id) REFERENCES user_accounts(user_id),
            FOREIGN KEY (grade_id) REFERENCES grades(grade_id)
        )",
        [],
    )?;

    // RoomComputers table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS room_computers (
            room_computer_id INTEGER PRIMARY KEY AUTOINCREMENT,
            computer_name TEXT NOT NULL,
            ip_address TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'Active' CHECK(status IN ('Active', 'Repairing', 'Broken'))
        )",
        [],
    )?;

    // PracticeSessions table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS practice_sessions (
            practice_session_id INTEGER PRIMARY KEY AUTOINCREMENT,
            practice_session_name TEXT NOT NULL,
            created_by_user_id INTEGER NOT NULL,
            grade_id INTEGER,
            subject_id INTEGER,
            school_year_id INTEGER,
            practice_time_slot_id INTEGER,
            status INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (created_by_user_id) REFERENCES user_accounts(user_id),
            FOREIGN KEY (grade_id) REFERENCES grades(grade_id),
            FOREIGN KEY (subject_id) REFERENCES subjects(subject_id),
            FOREIGN KEY (school_year_id) REFERENCES school_years(school_year_id),
            FOREIGN KEY (practice_time_slot_id) REFERENCES practice_time_slots(practice_time_slot_id)
        )",
        [],
    )?;

    // PracticeMessages table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS practice_messages (
            message_id INTEGER PRIMARY KEY AUTOINCREMENT,
            practice_session_id INTEGER NOT NULL,
            sender_user_id INTEGER NOT NULL,
            sender_name TEXT NOT NULL,
            receiver_user_id INTEGER,
            message_type TEXT NOT NULL DEFAULT 'All' CHECK(message_type IN ('Direct', 'All')),
            message_content TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (practice_session_id) REFERENCES practice_sessions(practice_session_id),
            FOREIGN KEY (sender_user_id) REFERENCES user_accounts(user_id)
        )",
        [],
    )?;

    // PracticeMaterials table
    conn.execute(
        "CREATE TABLE IF NOT EXISTS practice_materials (
            practice_material_id INTEGER PRIMARY KEY AUTOINCREMENT,
            practice_session_id INTEGER NOT NULL,
            material_title TEXT NOT NULL,
            material_file_path TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (practice_session_id) REFERENCES practice_sessions(practice_session_id)
        )",
        [],
    )?;

    Ok(())
}

fn seed_default_data(conn: &Connection) -> SqlResult<()> {
    // Check if users already exist
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM user_accounts",
        [],
        |row| row.get(0),
    )?;

    if count == 0 {
        // Create default users with hashed passwords
        let admin_hash = hash("admin123", DEFAULT_COST).unwrap();
        let teacher_hash = hash("teacher123", DEFAULT_COST).unwrap();
        let student_hash = hash("student123", DEFAULT_COST).unwrap();

        // Insert Administrator
        conn.execute(
            "INSERT INTO user_accounts (user_name, password_hash, role, status) VALUES (?1, ?2, ?3, 1)",
            params!["admin", admin_hash, "Administrator"],
        )?;

        // Insert Teacher
        conn.execute(
            "INSERT INTO user_accounts (user_name, password_hash, role, status) VALUES (?1, ?2, ?3, 1)",
            params!["teacher", teacher_hash, "Teacher"],
        )?;

        // Insert Student
        conn.execute(
            "INSERT INTO user_accounts (user_name, password_hash, role, status) VALUES (?1, ?2, ?3, 1)",
            params!["student", student_hash, "Student"],
        )?;

        // Create Teacher profile
        conn.execute(
            "INSERT INTO teachers (teacher_code, teacher_name, user_id) 
             SELECT 'GV001', 'Nguyễn Văn A', user_id FROM user_accounts WHERE user_name = 'teacher'",
            [],
        )?;

        // Create default school year
        conn.execute(
            "INSERT INTO school_years (school_year_name, start_date, end_date) VALUES ('2025-2026', '2025-09-01', '2026-06-30')",
            [],
        )?;

        // Create default grade
        conn.execute(
            "INSERT INTO grades (grade_name, school_year_id) VALUES ('Lớp 10A1', 1)",
            [],
        )?;

        // Create Student profile
        conn.execute(
            "INSERT INTO students (student_code, student_name, user_id, grade_id) 
             SELECT 'HS001', 'Trần Văn B', user_id, 1 FROM user_accounts WHERE user_name = 'student'",
            [],
        )?;

        println!("✅ Default users created:");
        println!("   - admin / admin123 (Administrator)");
        println!("   - teacher / teacher123 (Teacher)");
        println!("   - student / student123 (Student)");
    }

    Ok(())
}

// ============== Authentication Functions ==============
pub fn authenticate_user(conn: &Connection, username: &str, password: &str) -> LoginResponse {
    // Query user data
    let result = conn.query_row(
        "SELECT user_id, user_name, password_hash, role, status, created_at 
         FROM user_accounts WHERE user_name = ?1",
        params![username],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,      // user_id
                row.get::<_, String>(1)?,   // user_name
                row.get::<_, String>(2)?,   // password_hash
                row.get::<_, String>(3)?,   // role
                row.get::<_, i64>(4)?,      // status
                row.get::<_, String>(5)?,   // created_at
            ))
        },
    );

    match result {
        Ok((user_id, user_name, password_hash, role, status, created_at)) => {
            // Verify password
            match verify(password, &password_hash) {
                Ok(true) => {
                    if status != 1 {
                        return LoginResponse {
                            success: false,
                            message: "Tài khoản đã bị khóa".to_string(),
                            user: None,
                        };
                    }
                    
                    LoginResponse {
                        success: true,
                        message: "Đăng nhập thành công".to_string(),
                        user: Some(UserAccount {
                            user_id,
                            user_name,
                            role,
                            status: status == 1,
                            created_at,
                        }),
                    }
                }
                Ok(false) => LoginResponse {
                    success: false,
                    message: "Mật khẩu không đúng".to_string(),
                    user: None,
                },
                Err(_) => LoginResponse {
                    success: false,
                    message: "Lỗi xác thực".to_string(),
                    user: None,
                },
            }
        }
        Err(_) => LoginResponse {
            success: false,
            message: "Tài khoản không tồn tại".to_string(),
            user: None,
        },
    }
}

pub fn get_user_by_id(conn: &Connection, user_id: i64) -> SqlResult<Option<UserAccount>> {
    let result = conn.query_row(
        "SELECT user_id, user_name, role, status, created_at FROM user_accounts WHERE user_id = ?1",
        params![user_id],
        |row| {
            Ok(UserAccount {
                user_id: row.get(0)?,
                user_name: row.get(1)?,
                role: row.get(2)?,
                status: row.get::<_, i64>(3)? == 1,
                created_at: row.get(4)?,
            })
        },
    );

    match result {
        Ok(user) => Ok(Some(user)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn get_all_users(conn: &Connection) -> SqlResult<Vec<UserAccount>> {
    let mut stmt = conn.prepare(
        "SELECT user_id, user_name, role, status, created_at FROM user_accounts ORDER BY user_id"
    )?;
    
    let user_iter = stmt.query_map([], |row| {
        Ok(UserAccount {
            user_id: row.get(0)?,
            user_name: row.get(1)?,
            role: row.get(2)?,
            status: row.get::<_, i64>(3)? == 1,
            created_at: row.get(4)?,
        })
    })?;
    
    let mut users = Vec::new();
    for user in user_iter {
        users.push(user?);
    }
    
    Ok(users)
}

// ============== Device Functions (existing) ==============
pub fn save_device(conn: &Connection, device: &SavedDevice) -> SqlResult<i64> {
    conn.execute(
        "INSERT OR REPLACE INTO devices (ip, name, port, last_used) 
         VALUES (?1, ?2, ?3, ?4)",
        params![device.ip, device.name, device.port, device.last_used],
    )?;
    
    Ok(conn.last_insert_rowid())
}

pub fn get_all_devices(conn: &Connection) -> SqlResult<Vec<SavedDevice>> {
    let mut stmt = conn.prepare(
        "SELECT id, ip, name, port, last_used FROM devices ORDER BY last_used DESC"
    )?;
    
    let device_iter = stmt.query_map([], |row| {
        Ok(SavedDevice {
            id: Some(row.get(0)?),
            ip: row.get(1)?,
            name: row.get(2)?,
            port: row.get(3)?,
            last_used: row.get(4)?,
        })
    })?;
    
    let mut devices = Vec::new();
    for device in device_iter {
        devices.push(device?);
    }
    
    Ok(devices)
}

pub fn delete_device(conn: &Connection, id: i64) -> SqlResult<()> {
    conn.execute("DELETE FROM devices WHERE id = ?1", params![id])?;
    Ok(())
}

#[allow(dead_code)]
pub fn update_device_last_used(conn: &Connection, ip: &str, port: u16) -> SqlResult<()> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    
    conn.execute(
        "UPDATE devices SET last_used = ?1 WHERE ip = ?2 AND port = ?3",
        params![timestamp, ip, port],
    )?;
    
    Ok(())
}
