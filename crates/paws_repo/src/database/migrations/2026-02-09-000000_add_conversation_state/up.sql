-- Add state column to conversations table
ALTER TABLE conversations ADD COLUMN state TEXT NOT NULL DEFAULT 'idle';
