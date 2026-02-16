use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub conv_type: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Decision {
    pub id: String,
    pub conversation_id: String,
    pub title: String,
    pub status: String,
    pub summary_json: Option<String>,
    pub user_choice: Option<String>,
    pub user_choice_reasoning: Option<String>,
    pub outcome: Option<String>,
    pub outcome_date: Option<String>,
    pub debate_brief: Option<String>,
    pub debate_started_at: Option<String>,
    pub debate_completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DebateRound {
    pub id: String,
    pub decision_id: String,
    pub round_number: i32,
    pub exchange_number: i32,
    pub agent: String,
    pub content: String,
    pub created_at: String,
}

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new(path: &str) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        conn.execute_batch("
            CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                type TEXT NOT NULL DEFAULT 'chat',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (conversation_id) REFERENCES conversations(id)
            );
            CREATE TABLE IF NOT EXISTS decisions (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                title TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'exploring',
                summary_json TEXT,
                user_choice TEXT,
                user_choice_reasoning TEXT,
                outcome TEXT,
                outcome_date TEXT,
                debate_brief TEXT,
                debate_started_at TEXT,
                debate_completed_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (conversation_id) REFERENCES conversations(id)
            );
            CREATE TABLE IF NOT EXISTS debate_rounds (
                id TEXT PRIMARY KEY,
                decision_id TEXT NOT NULL,
                round_number INTEGER NOT NULL,
                exchange_number INTEGER DEFAULT 1,
                agent TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                FOREIGN KEY (decision_id) REFERENCES decisions(id)
            );
        ")?;

        // Migration: add type column if missing (existing databases)
        let has_type: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('conversations') WHERE name='type'")
            .and_then(|mut s| s.query_row([], |r| r.get::<_, i64>(0)))
            .map(|c| c > 0)
            .unwrap_or(false);
        if !has_type {
            conn.execute_batch("ALTER TABLE conversations ADD COLUMN type TEXT NOT NULL DEFAULT 'chat';")?;
        }

        // Migration: add debate columns to decisions table if missing
        let has_debate_brief: bool = conn
            .prepare("SELECT COUNT(*) FROM pragma_table_info('decisions') WHERE name='debate_brief'")
            .and_then(|mut s| s.query_row([], |r| r.get::<_, i64>(0)))
            .map(|c| c > 0)
            .unwrap_or(false);
        if !has_debate_brief {
            conn.execute_batch("
                ALTER TABLE decisions ADD COLUMN debate_brief TEXT;
                ALTER TABLE decisions ADD COLUMN debate_started_at TEXT;
                ALTER TABLE decisions ADD COLUMN debate_completed_at TEXT;
            ")?;
        }

        Ok(Self { conn: Mutex::new(conn) })
    }

    pub fn create_conversation(&self, title: &str) -> Result<Conversation, rusqlite::Error> {
        self.create_conversation_with_type(title, "chat")
    }

    pub fn create_conversation_with_type(&self, title: &str, conv_type: &str) -> Result<Conversation, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO conversations (id, title, type, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, title, conv_type, now, now],
        )?;
        Ok(Conversation { id, title: title.to_string(), conv_type: conv_type.to_string(), created_at: now.clone(), updated_at: now })
    }

    pub fn get_conversations(&self) -> Result<Vec<Conversation>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, title, type, created_at, updated_at FROM conversations ORDER BY updated_at DESC")?;
        let rows = stmt.query_map([], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                title: row.get(1)?,
                conv_type: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_conversations_by_type(&self, conv_type: &str) -> Result<Vec<Conversation>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, title, type, created_at, updated_at FROM conversations WHERE type = ?1 ORDER BY updated_at DESC")?;
        let rows = stmt.query_map(params![conv_type], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                title: row.get(1)?,
                conv_type: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_conversation(&self, conversation_id: &str) -> Result<Option<Conversation>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, title, type, created_at, updated_at FROM conversations WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![conversation_id], |row| {
            Ok(Conversation {
                id: row.get(0)?,
                title: row.get(1)?,
                conv_type: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn add_message(&self, conversation_id: &str, role: &str, content: &str) -> Result<Message, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, conversation_id, role, content, now],
        )?;
        conn.execute(
            "UPDATE conversations SET updated_at = ?1 WHERE id = ?2",
            params![now, conversation_id],
        )?;
        Ok(Message { id, conversation_id: conversation_id.to_string(), role: role.to_string(), content: content.to_string(), created_at: now })
    }

    pub fn get_messages(&self, conversation_id: &str) -> Result<Vec<Message>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, conversation_id, role, content, created_at FROM messages WHERE conversation_id = ?1 ORDER BY created_at ASC")?;
        let rows = stmt.query_map(params![conversation_id], |row| {
            Ok(Message {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn delete_conversation(&self, conversation_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM debate_rounds WHERE decision_id IN (SELECT id FROM decisions WHERE conversation_id = ?1)", params![conversation_id])?;
        conn.execute("DELETE FROM messages WHERE conversation_id = ?1", params![conversation_id])?;
        conn.execute("DELETE FROM decisions WHERE conversation_id = ?1", params![conversation_id])?;
        conn.execute("DELETE FROM conversations WHERE id = ?1", params![conversation_id])?;
        Ok(())
    }

    // ── Decision methods ──

    pub fn create_decision(&self, conversation_id: &str, title: &str) -> Result<Decision, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO decisions (id, conversation_id, title, status, created_at, updated_at) VALUES (?1, ?2, ?3, 'exploring', ?4, ?5)",
            params![id, conversation_id, title, now, now],
        )?;
        Ok(Decision {
            id,
            conversation_id: conversation_id.to_string(),
            title: title.to_string(),
            status: "exploring".to_string(),
            summary_json: None,
            user_choice: None,
            user_choice_reasoning: None,
            outcome: None,
            outcome_date: None,
            debate_brief: None,
            debate_started_at: None,
            debate_completed_at: None,
            created_at: now.clone(),
            updated_at: now,
        })
    }

    pub fn get_decisions(&self) -> Result<Vec<Decision>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, title, status, summary_json, user_choice, user_choice_reasoning, outcome, outcome_date, debate_brief, debate_started_at, debate_completed_at, created_at, updated_at FROM decisions ORDER BY updated_at DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Decision {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                title: row.get(2)?,
                status: row.get(3)?,
                summary_json: row.get(4)?,
                user_choice: row.get(5)?,
                user_choice_reasoning: row.get(6)?,
                outcome: row.get(7)?,
                outcome_date: row.get(8)?,
                debate_brief: row.get(9)?,
                debate_started_at: row.get(10)?,
                debate_completed_at: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })?;
        rows.collect()
    }

    pub fn get_decision(&self, decision_id: &str) -> Result<Option<Decision>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, title, status, summary_json, user_choice, user_choice_reasoning, outcome, outcome_date, debate_brief, debate_started_at, debate_completed_at, created_at, updated_at FROM decisions WHERE id = ?1"
        )?;
        let mut rows = stmt.query_map(params![decision_id], |row| {
            Ok(Decision {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                title: row.get(2)?,
                status: row.get(3)?,
                summary_json: row.get(4)?,
                user_choice: row.get(5)?,
                user_choice_reasoning: row.get(6)?,
                outcome: row.get(7)?,
                outcome_date: row.get(8)?,
                debate_brief: row.get(9)?,
                debate_started_at: row.get(10)?,
                debate_completed_at: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn get_decision_by_conversation(&self, conversation_id: &str) -> Result<Option<Decision>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, conversation_id, title, status, summary_json, user_choice, user_choice_reasoning, outcome, outcome_date, debate_brief, debate_started_at, debate_completed_at, created_at, updated_at FROM decisions WHERE conversation_id = ?1"
        )?;
        let mut rows = stmt.query_map(params![conversation_id], |row| {
            Ok(Decision {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                title: row.get(2)?,
                status: row.get(3)?,
                summary_json: row.get(4)?,
                user_choice: row.get(5)?,
                user_choice_reasoning: row.get(6)?,
                outcome: row.get(7)?,
                outcome_date: row.get(8)?,
                debate_brief: row.get(9)?,
                debate_started_at: row.get(10)?,
                debate_completed_at: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })?;
        match rows.next() {
            Some(row) => Ok(Some(row?)),
            None => Ok(None),
        }
    }

    pub fn update_decision_summary(&self, decision_id: &str, summary_json: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE decisions SET summary_json = ?1, updated_at = ?2 WHERE id = ?3",
            params![summary_json, now, decision_id],
        )?;
        Ok(())
    }

    pub fn update_decision_status(&self, decision_id: &str, status: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE decisions SET status = ?1, updated_at = ?2 WHERE id = ?3",
            params![status, now, decision_id],
        )?;
        Ok(())
    }

    pub fn update_decision_choice(&self, decision_id: &str, user_choice: &str, reasoning: Option<&str>) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE decisions SET status = 'decided', user_choice = ?1, user_choice_reasoning = ?2, updated_at = ?3 WHERE id = ?4",
            params![user_choice, reasoning.unwrap_or(""), now, decision_id],
        )?;
        Ok(())
    }

    pub fn update_decision_outcome(&self, decision_id: &str, outcome: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE decisions SET status = 'reviewed', outcome = ?1, outcome_date = ?2, updated_at = ?3 WHERE id = ?4",
            params![outcome, now, now, decision_id],
        )?;
        Ok(())
    }

    // ── Debate methods ──

    pub fn save_debate_round(
        &self,
        decision_id: &str,
        round_number: i32,
        exchange_number: i32,
        agent: &str,
        content: &str,
    ) -> Result<DebateRound, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO debate_rounds (id, decision_id, round_number, exchange_number, agent, content, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id, decision_id, round_number, exchange_number, agent, content, now],
        )?;
        Ok(DebateRound {
            id,
            decision_id: decision_id.to_string(),
            round_number,
            exchange_number,
            agent: agent.to_string(),
            content: content.to_string(),
            created_at: now,
        })
    }

    pub fn get_debate_rounds(&self, decision_id: &str) -> Result<Vec<DebateRound>, rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, decision_id, round_number, exchange_number, agent, content, created_at FROM debate_rounds WHERE decision_id = ?1 ORDER BY round_number ASC, exchange_number ASC, created_at ASC"
        )?;
        let rows = stmt.query_map(params![decision_id], |row| {
            Ok(DebateRound {
                id: row.get(0)?,
                decision_id: row.get(1)?,
                round_number: row.get(2)?,
                exchange_number: row.get(3)?,
                agent: row.get(4)?,
                content: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        rows.collect()
    }

    pub fn delete_debate_rounds(&self, decision_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM debate_rounds WHERE decision_id = ?1", params![decision_id])?;
        Ok(())
    }

    pub fn update_debate_brief(&self, decision_id: &str, brief: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE decisions SET debate_brief = ?1, updated_at = ?2 WHERE id = ?3",
            params![brief, now, decision_id],
        )?;
        Ok(())
    }

    pub fn update_debate_started(&self, decision_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE decisions SET status = 'debating', debate_started_at = ?1, debate_completed_at = NULL, updated_at = ?2 WHERE id = ?3",
            params![now, now, decision_id],
        )?;
        Ok(())
    }

    pub fn update_debate_completed(&self, decision_id: &str) -> Result<(), rusqlite::Error> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE decisions SET debate_completed_at = ?1, updated_at = ?2 WHERE id = ?3",
            params![now, now, decision_id],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decisions;
    use serde_json::json;

    fn new_test_db() -> Database {
        Database::new(":memory:").expect("in-memory database should initialize")
    }

    #[test]
    fn integration_creates_conversation_and_reads_messages() {
        let db = new_test_db();
        let conversation = db
            .create_conversation("Career planning")
            .expect("conversation should be created");

        db.add_message(&conversation.id, "user", "I need help deciding")
            .expect("user message should save");
        db.add_message(&conversation.id, "assistant", "Let's break this down")
            .expect("assistant message should save");

        let messages = db
            .get_messages(&conversation.id)
            .expect("messages should load");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[1].role, "assistant");
    }

    #[test]
    fn integration_delete_conversation_removes_messages_decision_and_debate_rounds() {
        let db = new_test_db();
        let conversation = db
            .create_conversation_with_type("Move cities?", "decision")
            .expect("decision conversation should be created");
        let decision = db
            .create_decision(&conversation.id, "Move cities?")
            .expect("decision should be created");

        db.add_message(&conversation.id, "user", "Thinking about relocating")
            .expect("message should save");
        db.save_debate_round(&decision.id, 1, 1, "rationalist", "Opening take")
            .expect("debate round should save");

        db.delete_conversation(&conversation.id)
            .expect("conversation should delete");

        assert!(
            db.get_conversation(&conversation.id)
                .expect("conversation query should succeed")
                .is_none()
        );
        assert_eq!(
            db.get_messages(&conversation.id)
                .expect("message query should succeed")
                .len(),
            0
        );
        assert!(
            db.get_decision(&decision.id)
                .expect("decision query should succeed")
                .is_none()
        );
        assert_eq!(
            db.get_debate_rounds(&decision.id)
                .expect("debate round query should succeed")
                .len(),
            0
        );
    }

    #[test]
    fn e2e_decision_lifecycle_from_exploring_to_reviewed() {
        let db = new_test_db();
        let conversation = db
            .create_conversation_with_type("Should I take the offer?", "decision")
            .expect("decision conversation should be created");
        let decision = db
            .create_decision(&conversation.id, "Should I take the offer?")
            .expect("decision should be created");

        db.add_message(&conversation.id, "user", "I have two job options")
            .expect("user message should save");
        db.add_message(&conversation.id, "assistant", "Let's compare tradeoffs")
            .expect("assistant message should save");

        let summary_update = json!({
            "options": [
                {"label": "Stay", "description": "Current job"},
                {"label": "Leave", "description": "New offer"}
            ],
            "variables": [
                {"label": "Compensation", "value": "20% increase", "impact": "high"}
            ]
        });
        let merged = decisions::merge_summary(None, &summary_update);
        db.update_decision_summary(&decision.id, &merged)
            .expect("summary should update");
        db.update_decision_status(&decision.id, "analyzing")
            .expect("status should update to analyzing");

        db.update_debate_started(&decision.id)
            .expect("debate should start");
        db.save_debate_round(&decision.id, 1, 1, "rationalist", "Option Leave has better EV")
            .expect("debate round should save");
        db.save_debate_round(&decision.id, 99, 1, "moderator", "Recommend Leave")
            .expect("moderator round should save");
        db.update_debate_completed(&decision.id)
            .expect("debate should complete");
        db.update_decision_status(&decision.id, "recommended")
            .expect("status should update to recommended");

        db.update_decision_choice(&decision.id, "Leave", Some("Better long-term growth"))
            .expect("choice should save");
        db.update_decision_outcome(&decision.id, "Took offer and it improved trajectory")
            .expect("outcome should save");

        let final_decision = db
            .get_decision(&decision.id)
            .expect("decision query should succeed")
            .expect("decision should exist");

        assert_eq!(final_decision.status, "reviewed");
        assert_eq!(final_decision.user_choice.as_deref(), Some("Leave"));
        assert_eq!(
            final_decision.outcome.as_deref(),
            Some("Took offer and it improved trajectory")
        );
        assert!(final_decision.debate_started_at.is_some());
        assert!(final_decision.debate_completed_at.is_some());
        assert!(final_decision.summary_json.is_some());

        let rounds = db
            .get_debate_rounds(&decision.id)
            .expect("debate rounds should load");
        assert_eq!(rounds.len(), 2);
        assert_eq!(rounds[0].round_number, 1);
        assert_eq!(rounds[1].round_number, 99);
    }
}
