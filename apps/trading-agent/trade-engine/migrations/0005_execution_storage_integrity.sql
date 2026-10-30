CREATE UNIQUE INDEX IF NOT EXISTS idx_execution_requests_risk_unique
    ON execution_requests(risk_evaluation_id)
    WHERE risk_evaluation_id IS NOT NULL;
