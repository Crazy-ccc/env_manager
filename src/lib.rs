use std::collections::HashMap;
use std::error::Error;
use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};
use std::cell::RefCell;
use std::rc::Rc;
use eframe::egui::Ui;
use eframe::{egui, Frame};

#[derive(PartialEq, Debug, Clone)]
pub struct Category {
    pub path: String,
    pub name: String,
    pub env_name: Option<String>,
    pub categories: Rc<RefCell<Option<Vec<Category>>>>,
    pub is_used: bool,
}

impl Category {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {
            path: String::new(),
            name: String::from(""),
            env_name: None,
            categories: Rc::new(RefCell::new(Some(
                read_current_dir(None).unwrap(),
            ))),
            is_used: false,
        }
    }
    pub fn build(&self, is_used: bool) -> Category {
        Category {
            is_used,
            ..self.clone()
        }
    }

    fn change_category(&self, category: Category) {
        let mut categories = self.categories.borrow_mut();
        let replace_categories: Vec<_> = categories
            .as_ref()
            .unwrap()
            .iter()
            .map(|c| c.build(c == &category))
            .collect();

        categories.replace(replace_categories);
    }

    pub fn get_categories(category: &Category) -> Option<Vec<Category>> {
        category.categories.borrow().clone()
    }

    pub fn set_env(&self) -> Result<(), Box<dyn Error>> {
        if let Some(env_name) = self.env_name.clone() {
            // // 校验Path中是否已配置
            // if let Ok(path_vars) = env::var("Path")
            //     && !path_vars.contains(env_name.as_str()) {
            //
            // }

            Command::new("setx").arg(env_name).arg(&self.path).spawn()?;
        }
        Ok(())
    }

    pub fn supported_lang() -> HashMap<String, String> {
        let mut supported_lang = HashMap::new();
        supported_lang.insert(String::from("java"), String::from("JAVA_HOME"));
        supported_lang.insert(String::from("scala"), String::from("SCALA_HOME"));
        supported_lang.insert(String::from("go"), String::from("GO_HOME"));

        supported_lang
    }
}

fn read_dir(path: PathBuf, ignore_file: bool, env_name: String) -> Vec<Category> {
    let mut results = vec![];

    let path_used = env::var(&env_name).unwrap();

    fs::read_dir(path)
        .unwrap_or_else(|e| panic!("{}", e))
        .for_each(|dir_entry| {
            let dir = dir_entry.unwrap_or_else(|error| {
                panic!("{}", error);
            });
            if ignore_file && dir.file_type().unwrap().is_file() {
                return;
            }
            let path = dir.path().to_str().unwrap().to_string();
            results.push(Category {
                path: path.clone(),
                name: dir.file_name().to_string_lossy().to_string(),
                env_name: Some(env_name.clone()),
                categories: Rc::new(RefCell::new(None)),
                is_used: path == path_used,
            })
        });

    results
}

pub fn read_current_dir(current_path: Option<PathBuf>) -> Result<Vec<Category>, Box<dyn Error>> {
    let mut results: Vec<Category> = Vec::new();
    let supported_lang = Category::supported_lang();

    let current_path = match current_path {
        Some(path) => path,
        None => env::current_dir()?,
    };

    fs::read_dir(current_path)?.for_each(|dir_entry| {
        let dir = dir_entry.unwrap();
        if dir.file_type().unwrap().is_file() {
            return;
        }
        let dir_name = dir.file_name().to_str().unwrap().trim().to_string();
        let dir_path = dir.path().to_str().unwrap().to_string();

        if supported_lang.contains_key(dir_name.clone().as_str()) {
            results.push(Category {
                path: dir_path.clone(),
                name: dir_name.clone(),
                env_name: None,
                categories: Rc::new(RefCell::new(Some(read_dir(
                    dir_path.parse().unwrap(),
                    true,
                    supported_lang.get(dir_name.as_str()).unwrap().to_string(),
                )))),
                is_used: false,
            })
        }
    });

    Ok(results)
}


impl eframe::App for Category {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut Frame) {
        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                Category::get_categories(self)
                    .unwrap()
                    .iter()
                    .for_each(|category| {
                        if ui
                            .add(egui::RadioButton::new(category.is_used, category.name.clone()))
                            .clicked()
                        {
                            self.change_category(category.clone());
                        }
                    });
            });

            if let Some(category) = Category::get_categories(self)
                .unwrap()
                .iter()
                .find(|c| c.is_used)
            {
                Category::get_categories(&category)
                    .unwrap()
                    .iter()
                    .for_each(|c| if ui.radio(c.is_used, c.name.clone()).clicked() {
                        c.set_env().expect("change env error!");
                        category.change_category(c.clone());
                    });
            }
        });
    }

    fn update(&mut self, _ctx: &egui::Context, _frame: &mut Frame) {}
}

#[cfg(test)]
mod tests {
    use crate::read_current_dir;

    #[test]
    fn test_read_current_dir() {
        assert_eq!(read_current_dir(None).unwrap().len(), 0);
    }

}
