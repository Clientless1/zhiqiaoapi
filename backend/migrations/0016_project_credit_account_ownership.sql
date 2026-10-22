-- Project funds move with the project. Personal key/model funds retain the
-- creator snapshot maintained by snapshot_credit_account_owner().
CREATE FUNCTION sync_project_credit_account_owner() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    UPDATE credit_account
    SET owner_user_id=NEW.owner_user_id, updated_at=now()
    WHERE owner_type='project' AND owner_id=NEW.id
      AND owner_user_id IS DISTINCT FROM NEW.owner_user_id;
    RETURN NEW;
END;
$$;

CREATE TRIGGER project_credit_account_owner_sync
AFTER UPDATE OF owner_user_id ON project
FOR EACH ROW
WHEN (OLD.owner_user_id IS DISTINCT FROM NEW.owner_user_id)
EXECUTE FUNCTION sync_project_credit_account_owner();

-- Accounts whose projects have been deleted keep their last owner snapshot.
UPDATE credit_account w
SET owner_user_id=p.owner_user_id, updated_at=now()
FROM project p
WHERE w.owner_type='project' AND w.owner_id=p.id
  AND w.owner_user_id IS DISTINCT FROM p.owner_user_id;
