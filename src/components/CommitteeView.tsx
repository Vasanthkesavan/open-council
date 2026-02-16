import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { FolderOpen, Users } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import ProfileFileList from "./ProfileFileList";
import ProfileFileContent from "./ProfileFileContent";

interface AgentFileInfo {
  filename: string;
  content: string;
  modified_at: string;
  size_bytes: number;
}

interface CommitteeViewProps {
  onNavigateToChat: () => void;
}

export default function CommitteeView({ onNavigateToChat }: CommitteeViewProps) {
  const [files, setFiles] = useState<AgentFileInfo[]>([]);
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadFiles();
  }, []);

  async function loadFiles() {
    try {
      const result = await invoke<AgentFileInfo[]>("get_agent_files");
      setFiles(result);
      // Auto-select first file if none selected
      if (!selectedFile && result.length > 0) {
        setSelectedFile(result[0].filename);
      }
    } catch (err) {
      console.error("Failed to load agent files:", err);
    } finally {
      setLoading(false);
    }
  }

  async function handleSave(filename: string, content: string) {
    await invoke<AgentFileInfo>("update_agent_file", { filename, content });
    await loadFiles();
  }

  async function handleOpenFolder() {
    try {
      const path = await invoke<string>("open_agents_folder");
      await revealItemInDir(path);
    } catch (err) {
      console.error("Failed to open folder:", err);
    }
  }

  const selectedFileData = files.find((f) => f.filename === selectedFile) ?? null;

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center">
        <p className="text-sm text-muted-foreground">Loading committee...</p>
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2 border-b border-border shrink-0">
        <h2 className="text-sm font-semibold">Committee Members</h2>
        <Button
          variant="ghost"
          size="icon"
          onClick={handleOpenFolder}
          className="h-8 w-8"
          title="Open agents folder"
        >
          <FolderOpen className="h-4 w-4" />
        </Button>
      </div>

      {/* Body */}
      {files.length === 0 ? (
        <div className="flex-1 flex items-center justify-center">
          <div className="text-center max-w-md px-4">
            <div className="mx-auto mb-4 h-12 w-12 rounded-2xl bg-accent-foreground/10 flex items-center justify-center">
              <Users className="h-6 w-6 text-foreground/70" />
            </div>
            <h2 className="text-xl font-semibold text-foreground/80 mb-2">
              No Committee Members
            </h2>
            <p className="text-muted-foreground text-sm leading-relaxed mb-6">
              Committee member prompts will be created when you start your first debate.
            </p>
            <Button onClick={onNavigateToChat}>Start a Decision</Button>
          </div>
        </div>
      ) : (
        <div className="flex-1 flex min-h-0">
          {/* Left panel - agent list */}
          <div className="w-[250px] border-r border-border shrink-0 bg-muted/10">
            <ScrollArea className="h-full">
              <ProfileFileList
                files={files}
                selectedFile={selectedFile}
                onSelect={setSelectedFile}
              />
            </ScrollArea>
          </div>

          {/* Right panel - prompt content */}
          <div className="flex-1 min-w-0 min-h-0">
            {selectedFileData ? (
              <ProfileFileContent
                key={selectedFileData.filename}
                file={selectedFileData}
                onSave={handleSave}
              />
            ) : (
              <div className="h-full flex items-center justify-center">
                <p className="text-sm text-muted-foreground">Select a member to view their prompt</p>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
