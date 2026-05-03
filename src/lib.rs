use std::error::Error;
use std::path::PathBuf;
use std::process::Command;
use std::{env, fs, io};

pub struct Config {
    pub path: String,
    pub name: String,
    pub env_name: String,
}

impl Config {
    pub fn set_env(&self) -> Result<(), Box<dyn Error>> {
        Command::new("setx").arg(&self.env_name).arg(&self.path).spawn()?;

        Ok(())
    }
}

pub fn read_dir(path: PathBuf, ignore_file: bool, env_name: String) -> Vec<Config> {
    let mut results = vec![];

    fs::read_dir(path)
        .unwrap_or_else(|e| panic!("{}", e))
        .for_each(|dir_entry| {
            let dir = dir_entry.unwrap_or_else(|error| {
                panic!("{}", error);
            });
            if ignore_file && dir.file_type().unwrap().is_file() {
                return;
            }
            results.push(Config {
                path: dir.path().to_str().unwrap().to_string(),
                name: dir.file_name().to_string_lossy().to_string(),
                env_name: env_name.clone()
            })
        });

    results
}

pub fn enter_number() -> usize {
    let mut number = String::new();
    io::stdin()
        .read_line(&mut number)
        .expect("Please enter a number!");

    number.trim().parse().expect("Please enter a number!")
}

pub fn out_dirs(dirs: &Vec<Config>) {
    for (i, config) in dirs.iter().enumerate() {
        println!("{} {}", i + 1, config.name);
    }
}

pub fn run() -> Result<(), Box<dyn Error>> {
    // 读取目录
    let current_path = env::current_dir()?;

    // 获取不同类型的配置信息
    let first_dirs = read_dir(current_path, true, String::new());
    println!("当前可用类型为:");
    out_dirs(&first_dirs);

    // 执行操作(选择操作 -> 修改环境变量)
    println!("请输入您所需要设置的类型:");
    let number = enter_number();
    let config = first_dirs.get(number - 1).unwrap();

    let second_dirs = read_dir(config.path.parse()?,
                               true,
                               config.name.to_uppercase() + "_HOME");
    println!("当前类型 {} 可供选择的选项为:", config.name);
    out_dirs(&second_dirs);
    println!("请输入您所需修改环境变量的选项:");
    let number = enter_number();
    let second_config = second_dirs.get(number - 1).unwrap();

    second_config.set_env()?;

    println!(
        "环境变量 {} 已修改为: {}",
        second_config.env_name,
        second_config.path
    );

    Ok(())
}

#[cfg(test)]
mod tests {}
