use crate::config::Config;
use crate::error::{Result, ModelSrvError};
use crate::model::{ModelDefinition, ModelEngine, ModelWithActions};
use crate::redis_handler::RedisConnection;
use crate::template::{TemplateInfo, TemplateManager};
use crate::control::{ControlOperation, ControlManager};
use crate::storage::DataStore;
use crate::storage_agent::StorageAgent;

use iced::{
    widget::{button, column, container, row, scrollable, text, text_input, Column, Stack},
    Element, Length, Theme, Alignment,
};
use iced::Application;
use iced::Command;
use iced::Settings;
use iced_aw::{Card, TabBar, TabLabel};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use log::{debug, error, info, warn};
use serde_json;

// Define message types for the GUI
#[derive(Debug, Clone)]
pub enum Message {
    // Tab navigation
    TabSelected(usize),
    
    // Template management
    TemplateSelected(String),
    TemplateNameChanged(String),
    CreateInstance,
    InstanceIdChanged(String),
    InstanceNameChanged(String),
    
    // Model monitoring
    RefreshModels,
    ModelSelected(String),
    
    // Control operations
    ControlOperationSelected(String),
    ExecuteOperation,
    
    // Background tasks
    BackgroundTaskCompleted(Result<BackgroundTaskResult>),
    
    // Error handling
    ErrorOccurred(String),
    DismissError,
    
    // Storage operations
    SyncToRedis,
}

// Background task results
#[derive(Debug, Clone)]
pub enum BackgroundTaskResult {
    TemplatesLoaded(Vec<TemplateInfo>),
    InstanceCreated(String),
    ModelsLoaded(Vec<String>),
    ModelDetailsLoaded(ModelWithActions),
    ControlOperationsLoaded(Vec<ControlOperation>),
    OperationExecuted(String),
    SyncCompleted,
}

// Main application state
pub struct ModsrvGui {
    // Configuration
    config: Config,
    
    // UI state
    active_tab: usize,
    error_message: Option<String>,
    
    // Template management
    templates: Vec<TemplateInfo>,
    selected_template: Option<String>,
    new_instance_id: String,
    new_instance_name: String,
    
    // Model monitoring
    models: Vec<String>,
    selected_model: Option<String>,
    model_details: Option<ModelWithActions>,
    
    // Control operations
    control_operations: Vec<ControlOperation>,
    selected_operation: Option<String>,
    
    // Background task channel
    task_sender: mpsc::Sender<BackgroundTask>,
    task_receiver: Arc<Mutex<mpsc::Receiver<BackgroundTask>>>,
    
    // Storage agent
    storage_agent: Arc<StorageAgent>,
}

// Background tasks
#[derive(Debug)]
enum BackgroundTask {
    LoadTemplates,
    CreateInstance(String, String, Option<String>),
    LoadModels,
    LoadModelDetails(String),
    LoadControlOperations,
    ExecuteOperation(String),
    SyncToRedis,
}

impl Application for ModsrvGui {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = Config;

    fn new(config: Config) -> (Self, Command<Message>) {
        // Create channel for background tasks
        let (task_sender, task_receiver) = mpsc::channel(100);
        
        // Create storage agent
        let storage_agent = match StorageAgent::new(config.clone()) {
            Ok(agent) => Arc::new(agent),
            Err(e) => {
                error!("Failed to create storage agent: {}", e);
                panic!("Failed to create storage agent: {}", e);
            }
        };
        
        let app = ModsrvGui {
            config,
            active_tab: 0,
            error_message: None,
            templates: Vec::new(),
            selected_template: None,
            new_instance_id: String::new(),
            new_instance_name: String::new(),
            models: Vec::new(),
            selected_model: None,
            model_details: None,
            control_operations: Vec::new(),
            selected_operation: None,
            task_sender,
            task_receiver: Arc::new(Mutex::new(task_receiver)),
            storage_agent,
        };
        
        // Initial command to load templates
        let command = command::Command::perform(
            load_templates(app.config.clone(), app.storage_agent.clone()),
            |result| match result {
                Ok(templates) => Message::BackgroundTaskCompleted(Ok(BackgroundTaskResult::TemplatesLoaded(templates))),
                Err(e) => Message::ErrorOccurred(format!("Failed to load templates: {}", e)),
            }
        );
        
        (app, command)
    }

    fn title(&self) -> String {
        String::from("Model Service - Visualization Interface")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::TabSelected(tab_index) => {
                self.active_tab = tab_index;
                
                // Load data for the selected tab
                match tab_index {
                    0 => command::Command::perform(
                        load_templates(self.config.clone(), self.storage_agent.clone()),
                        |result| match result {
                            Ok(templates) => Message::BackgroundTaskCompleted(Ok(BackgroundTaskResult::TemplatesLoaded(templates))),
                            Err(e) => Message::ErrorOccurred(format!("Failed to load templates: {}", e)),
                        }
                    ),
                    1 => command::Command::perform(
                        load_models(self.config.clone(), self.storage_agent.clone()),
                        |result| match result {
                            Ok(models) => Message::BackgroundTaskCompleted(Ok(BackgroundTaskResult::ModelsLoaded(models))),
                            Err(e) => Message::ErrorOccurred(format!("Failed to load models: {}", e)),
                        }
                    ),
                    2 => command::Command::perform(
                        load_control_operations(self.config.clone(), self.storage_agent.clone()),
                        |result| match result {
                            Ok(operations) => Message::BackgroundTaskCompleted(Ok(BackgroundTaskResult::ControlOperationsLoaded(operations))),
                            Err(e) => Message::ErrorOccurred(format!("Failed to load control operations: {}", e)),
                        }
                    ),
                    _ => command::Command::none(),
                }
            },
            
            // Template management
            Message::TemplateSelected(template_id) => {
                self.selected_template = Some(template_id);
                command::Command::none()
            },
            Message::TemplateNameChanged(name) => {
                self.new_instance_name = name;
                command::Command::none()
            },
            Message::InstanceIdChanged(id) => {
                self.new_instance_id = id;
                command::Command::none()
            },
            Message::InstanceNameChanged(name) => {
                self.new_instance_name = name;
                command::Command::none()
            },
            Message::CreateInstance => {
                if let Some(template_id) = &self.selected_template {
                    if !self.new_instance_id.is_empty() {
                        let instance_name = if self.new_instance_name.is_empty() {
                            None
                        } else {
                            Some(self.new_instance_name.clone())
                        };
                        
                        let template_id = template_id.clone();
                        let instance_id = self.new_instance_id.clone();
                        let storage_agent = self.storage_agent.clone();
                        
                        command::Command::perform(
                            async move {
                                create_instance(
                                    storage_agent,
                                    &template_id,
                                    &instance_id,
                                    instance_name.as_deref(),
                                ).await
                            },
                            |result| match result {
                                Ok(instance_id) => Message::BackgroundTaskCompleted(Ok(BackgroundTaskResult::InstanceCreated(instance_id))),
                                Err(e) => Message::ErrorOccurred(format!("Failed to create instance: {}", e)),
                            }
                        )
                    } else {
                        self.error_message = Some("Instance ID cannot be empty".to_string());
                        command::Command::none()
                    }
                } else {
                    self.error_message = Some("No template selected".to_string());
                    command::Command::none()
                }
            },
            
            // Model monitoring
            Message::RefreshModels => {
                command::Command::perform(
                    load_models(self.config.clone(), self.storage_agent.clone()),
                    |result| match result {
                        Ok(models) => Message::BackgroundTaskCompleted(Ok(BackgroundTaskResult::ModelsLoaded(models))),
                        Err(e) => Message::ErrorOccurred(format!("Failed to load models: {}", e)),
                    }
                )
            },
            Message::ModelSelected(model_id) => {
                self.selected_model = Some(model_id.clone());
                
                let storage_agent = self.storage_agent.clone();
                command::Command::perform(
                    async move {
                        load_model_details(storage_agent, &model_id).await
                    },
                    |result| match result {
                        Ok(model) => Message::BackgroundTaskCompleted(Ok(BackgroundTaskResult::ModelDetailsLoaded(model))),
                        Err(e) => Message::ErrorOccurred(format!("Failed to load model details: {}", e)),
                    }
                )
            },
            
            // Control operations
            Message::ControlOperationSelected(operation_id) => {
                self.selected_operation = Some(operation_id);
                command::Command::none()
            },
            Message::ExecuteOperation => {
                if let Some(operation_id) = &self.selected_operation {
                    let operation_id = operation_id.clone();
                    let storage_agent = self.storage_agent.clone();
                    
                    command::Command::perform(
                        async move {
                            execute_operation(storage_agent, &operation_id).await
                        },
                        |result| match result {
                            Ok(result) => Message::BackgroundTaskCompleted(Ok(BackgroundTaskResult::OperationExecuted(result))),
                            Err(e) => Message::ErrorOccurred(format!("Failed to execute operation: {}", e)),
                        }
                    )
                } else {
                    self.error_message = Some("No operation selected".to_string());
                    command::Command::none()
                }
            },
            
            // Storage operations
            Message::SyncToRedis => {
                let storage_agent = self.storage_agent.clone();
                
                command::Command::perform(
                    async move {
                        storage_agent.sync_to_redis()?;
                        Ok(())
                    },
                    |result: Result<()>| match result {
                        Ok(_) => Message::BackgroundTaskCompleted(Ok(BackgroundTaskResult::SyncCompleted)),
                        Err(e) => Message::ErrorOccurred(format!("Failed to sync to Redis: {}", e)),
                    }
                )
            },
            
            // Background task results
            Message::BackgroundTaskCompleted(result) => {
                match result {
                    Ok(BackgroundTaskResult::TemplatesLoaded(templates)) => {
                        self.templates = templates;
                        command::Command::none()
                    },
                    Ok(BackgroundTaskResult::InstanceCreated(instance_id)) => {
                        info!("Instance created: {}", instance_id);
                        self.new_instance_id = String::new();
                        self.new_instance_name = String::new();
                        command::Command::none()
                    },
                    Ok(BackgroundTaskResult::ModelsLoaded(models)) => {
                        self.models = models;
                        command::Command::none()
                    },
                    Ok(BackgroundTaskResult::ModelDetailsLoaded(model)) => {
                        self.model_details = Some(model);
                        command::Command::none()
                    },
                    Ok(BackgroundTaskResult::ControlOperationsLoaded(operations)) => {
                        self.control_operations = operations;
                        command::Command::none()
                    },
                    Ok(BackgroundTaskResult::OperationExecuted(result)) => {
                        info!("Operation executed: {}", result);
                        command::Command::none()
                    },
                    Ok(BackgroundTaskResult::SyncCompleted) => {
                        info!("Data synced to Redis successfully");
                        command::Command::none()
                    },
                    Err(e) => {
                        self.error_message = Some(format!("Error: {}", e));
                        command::Command::none()
                    }
                }
            },
            
            // Error handling
            Message::ErrorOccurred(message) => {
                self.error_message = Some(message);
                command::Command::none()
            },
            Message::DismissError => {
                self.error_message = None;
                command::Command::none()
            },
        }
    }

    fn view(&self) -> Element<Message> {
        // Create tab bar
        let tab_bar = TabBar::new(self.active_tab, Message::TabSelected)
            .push(TabLabel::Text("Templates".to_string()))
            .push(TabLabel::Text("Models".to_string()))
            .push(TabLabel::Text("Control".to_string()));
        
        // Add sync button if using Redis
        let header = if self.config.use_redis {
            row![
                text("Model Service").size(28),
                button("Sync to Redis").on_press(Message::SyncToRedis),
            ]
            .spacing(20)
            .align_items(Alignment::Center)
            .width(Length::Fill)
        } else {
            row![
                text("Model Service").size(28),
            ]
            .spacing(20)
            .align_items(Alignment::Center)
            .width(Length::Fill)
        };
        
        // Create content based on active tab
        let content = match self.active_tab {
            0 => self.view_templates_tab(),
            1 => self.view_models_tab(),
            2 => self.view_control_tab(),
            _ => column![text("Invalid tab")].into(),
        };
        
        // Create main layout
        let main_content = column![
            header,
            tab_bar,
            content,
        ]
        .spacing(20)
        .padding(20);
        
        // Show error modal if there's an error
        if let Some(error) = &self.error_message {
            // Use Stack instead of Modal
            Stack::new()
                .push(
                    container(main_content)
                        .width(Length::Fill)
                        .height(Length::Fill)
                )
                .push(
                    container(
                        Card::new(
                            text("Error"),
                            column![
                                text(error),
                                button("Dismiss").on_press(Message::DismissError),
                            ]
                            .spacing(10)
                            .padding(10)
                        )
                        .max_width(300.0)
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .center_x()
                    .center_y()
                )
                .into()
        } else {
            container(main_content)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x()
                .center_y()
                .into()
        }
    }
}

impl ModsrvGui {
    // View for Templates tab
    fn view_templates_tab(&self) -> Element<Message> {
        let templates_list = if self.templates.is_empty() {
            column![text("No templates available")]
        } else {
            let mut col = Column::new().spacing(5);
            
            for template in &self.templates {
                let is_selected = self.selected_template.as_ref() == Some(&template.id);
                
                let template_row = row![
                    text(&template.name).width(Length::Fill),
                    button(if is_selected { "Selected" } else { "Select" })
                        .on_press(Message::TemplateSelected(template.id.clone())),
                ]
                .spacing(10)
                .padding(5)
                .width(Length::Fill);
                
                col = col.push(template_row);
            }
            
            col
        };
        
        let templates_card = Card::new(
            text("Available Templates").size(24),
            scrollable(templates_list).height(Length::Fill),
        )
        .width(Length::Fill)
        .height(300);
        
        let instance_form = column![
            text("Create New Instance").size(20),
            row![
                text("Template:").width(Length::FillPortion(1)),
                text(self.selected_template.clone().unwrap_or_else(|| "None selected".to_string()))
                    .width(Length::FillPortion(3)),
            ].spacing(10),
            row![
                text("Instance ID:").width(Length::FillPortion(1)),
                text_input("Enter instance ID", &self.new_instance_id)
                    .on_input(Message::InstanceIdChanged)
                    .width(Length::FillPortion(3)),
            ].spacing(10),
            row![
                text("Instance Name:").width(Length::FillPortion(1)),
                text_input("Enter instance name (optional)", &self.new_instance_name)
                    .on_input(Message::InstanceNameChanged)
                    .width(Length::FillPortion(3)),
            ].spacing(10),
            button("Create Instance")
                .on_press(Message::CreateInstance)
                .width(Length::Fill),
        ]
        .spacing(10)
        .padding(10);
        
        let instance_card = Card::new(
            text("Create Instance").size(24),
            instance_form,
        )
        .width(Length::Fill);
        
        column![
            templates_card,
            instance_card,
        ]
        .spacing(20)
        .padding(20)
        .into()
    }
    
    // View for Models tab
    fn view_models_tab(&self) -> Element<Message> {
        let models_list = if self.models.is_empty() {
            column![text("No models available")]
        } else {
            let mut col = Column::new().spacing(5);
            
            for model_id in &self.models {
                let is_selected = self.selected_model.as_ref() == Some(model_id);
                
                let model_row = row![
                    text(model_id).width(Length::Fill),
                    button(if is_selected { "Selected" } else { "Select" })
                        .on_press(Message::ModelSelected(model_id.clone())),
                ]
                .spacing(10)
                .padding(5)
                .width(Length::Fill);
                
                col = col.push(model_row);
            }
            
            col
        };
        
        let models_card = Card::new(
            row![
                text("Running Models").size(24).width(Length::Fill),
                button("Refresh").on_press(Message::RefreshModels),
            ],
            scrollable(models_list).height(Length::Fill),
        )
        .width(Length::Fill)
        .height(300);
        
        let model_details = if let Some(model) = &self.model_details {
            column![
                row![
                    text("ID:").width(Length::FillPortion(1)),
                    text(&model.model.id).width(Length::FillPortion(3)),
                ].spacing(10),
                row![
                    text("Name:").width(Length::FillPortion(1)),
                    text(&model.model.name).width(Length::FillPortion(3)),
                ].spacing(10),
                row![
                    text("Description:").width(Length::FillPortion(1)),
                    text(&model.model.description).width(Length::FillPortion(3)),
                ].spacing(10),
                row![
                    text("Output Key:").width(Length::FillPortion(1)),
                    text(&model.model.output_key).width(Length::FillPortion(3)),
                ].spacing(10),
                text("Input Mappings:").size(16),
                {
                    let mut col = Column::new().spacing(5);
                    
                    for mapping in &model.model.input_mappings {
                        col = col.push(
                            row![
                                text(&mapping.field).width(Length::FillPortion(1)),
                                text("->").width(Length::FillPortion(1)),
                                text(&mapping.source_key).width(Length::FillPortion(2)),
                                text(&mapping.source_field).width(Length::FillPortion(2)),
                            ]
                            .spacing(5)
                        );
                    }
                    
                    scrollable(col).height(100)
                },
                text("Available Actions:").size(16),
                {
                    let mut col = Column::new().spacing(5);
                    
                    for action in &model.actions {
                        col = col.push(
                            row![
                                text(&action.name).width(Length::FillPortion(2)),
                                text(&action.channel).width(Length::FillPortion(2)),
                            ]
                            .spacing(5)
                        );
                    }
                    
                    scrollable(col).height(100)
                },
            ]
            .spacing(10)
            .padding(10)
        } else {
            column![text("No model selected")]
        };
        
        let model_details_card = Card::new(
            text("Model Details").size(24),
            model_details,
        )
        .width(Length::Fill);
        
        column![
            models_card,
            model_details_card,
        ]
        .spacing(20)
        .padding(20)
        .into()
    }
    
    // View for Control tab
    fn view_control_tab(&self) -> Element<Message> {
        let operations_list = if self.control_operations.is_empty() {
            column![text("No control operations available")]
        } else {
            let mut col = Column::new().spacing(5);
            
            for operation in &self.control_operations {
                let is_selected = self.selected_operation.as_ref() == Some(&operation.id);
                
                let operation_row = row![
                    text(&operation.name).width(Length::FillPortion(2)),
                    text(format!("{:?}", operation.operation_type)).width(Length::FillPortion(1)),
                    text(format!("{:?}", operation.target_type)).width(Length::FillPortion(1)),
                    text(&operation.target_id).width(Length::FillPortion(1)),
                    button(if is_selected { "Selected" } else { "Select" })
                        .on_press(Message::ControlOperationSelected(operation.id.clone())),
                ]
                .spacing(10)
                .padding(5)
                .width(Length::Fill);
                
                col = col.push(operation_row);
            }
            
            col
        };
        
        let operations_card = Card::new(
            text("Control Operations").size(24),
            scrollable(operations_list).height(Length::Fill),
        )
        .width(Length::Fill)
        .height(300);
        
        let execute_card = Card::new(
            text("Execute Operation").size(24),
            column![
                text(format!("Selected Operation: {}", self.selected_operation.clone().unwrap_or_else(|| "None".to_string()))),
                button("Execute")
                    .on_press(Message::ExecuteOperation)
                    .width(Length::Fill),
            ]
            .spacing(10)
            .padding(10),
        )
        .width(Length::Fill);
        
        column![
            operations_card,
            execute_card,
        ]
        .spacing(20)
        .padding(20)
        .into()
    }
}

// Background task functions
async fn load_templates(config: Config, storage_agent: Arc<StorageAgent>) -> Result<Vec<TemplateInfo>> {
    let template_manager = TemplateManager::new(&config.templates_dir, &config.redis.key_prefix);
    template_manager.list_templates()
}

async fn create_instance(
    storage_agent: Arc<StorageAgent>,
    template_id: &str,
    instance_id: &str,
    instance_name: Option<&str>,
) -> Result<String> {
    let store = storage_agent.store();
    let mut template_manager = TemplateManager::new(
        &storage_agent.store().memory_store().to_string(),
        &storage_agent.config.redis.key_prefix,
    );
    
    template_manager.create_instance(
        &*store,
        template_id,
        instance_id,
        instance_name,
    )?;
    
    Ok(instance_id.to_string())
}

async fn load_models(config: Config, storage_agent: Arc<StorageAgent>) -> Result<Vec<String>> {
    let store = storage_agent.store();
    
    let pattern = format!("{}model:config:*", config.redis.key_prefix);
    let keys = store.get_keys(&pattern)?;
    
    let mut model_ids = Vec::new();
    for key in keys {
        let parts: Vec<&str> = key.split(':').collect();
        if parts.len() >= 3 {
            model_ids.push(parts[2].to_string());
        }
    }
    
    Ok(model_ids)
}

async fn load_model_details(storage_agent: Arc<StorageAgent>, model_id: &str) -> Result<ModelWithActions> {
    let store = storage_agent.store();
    
    let key = format!("{}model:config:{}", storage_agent.config.redis.key_prefix, model_id);
    let json_str = store.get_string(&key)?;
    let model: ModelWithActions = serde_json::from_str(&json_str)?;
    
    Ok(model)
}

async fn load_control_operations(config: Config, storage_agent: Arc<StorageAgent>) -> Result<Vec<ControlOperation>> {
    let store = storage_agent.store();
    
    let pattern = format!("{}control:operation:*", config.redis.key_prefix);
    let keys = store.get_keys(&pattern)?;
    
    let mut operations = Vec::new();
    for key in &keys {
        match store.get_string(key).and_then(|json_str| {
            serde_json::from_str::<ControlOperation>(&json_str)
                .map_err(|e| ModelSrvError::JsonError(e))
        }) {
            Ok(operation) => operations.push(operation),
            Err(e) => error!("Failed to load control operation from {}: {}", key, e),
        }
    }
    
    Ok(operations)
}

async fn execute_operation(storage_agent: Arc<StorageAgent>, operation_id: &str) -> Result<String> {
    let store = storage_agent.store();
    
    let key = format!("{}control:operation:{}", storage_agent.config.redis.key_prefix, operation_id);
    let json_str = store.get_string(&key)?;
    let operation: ControlOperation = serde_json::from_str(&json_str)?;
    
    let mut control_manager = ControlManager::new(&storage_agent.config.redis.key_prefix);
    
    // This is a simplified version - in a real implementation, we would need to properly
    // initialize the control manager and execute the operation
    
    Ok(format!("Operation {} executed", operation_id))
}

// Public function to run the GUI
pub fn run_gui(config: Config) -> Result<()> {
    let settings = Settings::with_flags(config);
    ModsrvGui::run(settings)?;
    Ok(())
} 