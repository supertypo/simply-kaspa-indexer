use sqlx::{AssertSqlSafe, Error, Pool, Postgres};

fn split_statements(sql: &str) -> Vec<&str> {
    let bytes = sql.as_bytes();
    let len = bytes.len();
    let mut statements: Vec<&str> = Vec::new();
    let mut i = 0;
    let mut start = 0;

    while i < len {
        match bytes[i] {
            // Dollar-quote: $tag$...$tag$
            b'$' => {
                let tag_start = i;
                i += 1;
                while i < len && bytes[i] != b'$' {
                    i += 1;
                }
                if i < len {
                    i += 1;
                }
                let tag = &bytes[tag_start..i];
                loop {
                    if i >= len {
                        break;
                    }
                    if bytes[i] == b'$' {
                        let close_start = i;
                        i += 1;
                        while i < len && bytes[i] != b'$' {
                            i += 1;
                        }
                        if i < len {
                            i += 1;
                        }
                        if bytes[close_start..i] == *tag {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            // Single-quoted string
            b'\'' => {
                i += 1;
                while i < len {
                    if bytes[i] == b'\'' {
                        i += 1;
                        if i < len && bytes[i] == b'\'' {
                            i += 1; // escaped ''
                        } else {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
            // Line comment
            b'-' if i + 1 < len && bytes[i + 1] == b'-' => {
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            // Block comment
            b'/' if i + 1 < len && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                if i + 1 < len {
                    i += 2;
                }
            }
            // Statement terminator
            b';' => {
                let stmt = sql[start..i].trim();
                if !stmt.is_empty() {
                    statements.push(stmt);
                }
                i += 1;
                start = i;
            }
            _ => {
                i += 1;
            }
        }
    }
    let tail = sql[start..].trim();
    if !tail.is_empty() {
        statements.push(tail);
    }
    statements
}

pub async fn execute_ddl(ddl: &str, pool: &Pool<Postgres>) -> Result<(), Error> {
    let mut conn = pool.acquire().await?;
    for statement in split_statements(ddl) {
        sqlx::raw_sql(AssertSqlSafe(statement)).execute(&mut *conn).await?;
    }
    Ok(())
}
