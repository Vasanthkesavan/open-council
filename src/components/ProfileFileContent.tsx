import { useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import remarkBreaks from "remark-breaks";
import { Pencil, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { ScrollArea } from "@/components/ui/scroll-area";

interface ProfileFileInfo {
  filename: string;
  content: string;
  modified_at: string;
  size_bytes: number;
}

interface ProfileFileContentProps {
  file: ProfileFileInfo;
  onSave: (filename: string, content: string) => Promise<void>;
  onDelete?: (filename: string) => void;
}

export default function ProfileFileContent({
  file,
  onSave,
  onDelete,
}: ProfileFileContentProps) {
  const [isEditing, setIsEditing] = useState(false);
  const [editContent, setEditContent] = useState("");
  const [saving, setSaving] = useState(false);

  const displayName = file.filename.replace(/\.md$/, "");

  function handleEdit() {
    setEditContent(file.content);
    setIsEditing(true);
  }

  function handleCancel() {
    setIsEditing(false);
    setEditContent("");
  }

  async function handleSave() {
    setSaving(true);
    try {
      await onSave(file.filename, editContent);
      setIsEditing(false);
      setEditContent("");
    } catch (err) {
      console.error("Failed to save file:", err);
    } finally {
      setSaving(false);
    }
  }

  function formatModifiedDate(dateStr: string) {
    const date = new Date(dateStr);
    return date.toLocaleDateString(undefined, {
      year: "numeric",
      month: "long",
      day: "numeric",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="flex items-center justify-between px-6 py-3 border-b border-border shrink-0">
        <h2 className="text-sm font-semibold truncate">{displayName}</h2>
        <div className="flex items-center gap-1">
          {isEditing ? (
            <>
              <Button variant="ghost" size="sm" onClick={handleCancel} disabled={saving}>
                Cancel
              </Button>
              <Button size="sm" onClick={handleSave} disabled={saving}>
                {saving ? "Saving..." : "Save"}
              </Button>
            </>
          ) : (
            <>
              <Button
                variant="ghost"
                size="icon"
                onClick={handleEdit}
                className="h-8 w-8"
                title="Edit"
              >
                <Pencil className="h-4 w-4" />
              </Button>
              {onDelete && (
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => onDelete(file.filename)}
                  className="h-8 w-8 text-destructive hover:text-destructive"
                  title="Delete"
                >
                  <Trash2 className="h-4 w-4" />
                </Button>
              )}
            </>
          )}
        </div>
      </div>

      {/* Content */}
      {isEditing ? (
        <div className="flex-1 p-6 min-h-0">
          <Textarea
            value={editContent}
            onChange={(e) => setEditContent(e.target.value)}
            className="h-full resize-none font-mono text-sm"
          />
        </div>
      ) : (
        <ScrollArea className="flex-1">
          <div className="px-6 py-4">
            <div className="text-sm leading-relaxed text-foreground [&_p]:my-2 [&_strong]:font-semibold [&_a]:text-blue-600 dark:[&_a]:text-blue-400 [&_ul]:my-2 [&_ul]:list-disc [&_ul]:pl-6 [&_ol]:my-2 [&_ol]:list-decimal [&_ol]:pl-6 [&_li]:my-1 [&_li>p]:my-0 [&_pre]:my-2 [&_pre]:rounded-lg [&_pre]:border [&_pre]:border-border [&_pre]:bg-background [&_pre]:p-3 [&_pre]:overflow-x-auto [&_code:not(pre_code)]:rounded [&_code:not(pre_code)]:bg-muted/50 [&_code:not(pre_code)]:px-1 [&_code:not(pre_code)]:py-0.5 [&_code]:text-foreground/85 [&_h1]:text-lg [&_h1]:font-semibold [&_h1]:my-3 [&_h2]:text-base [&_h2]:font-semibold [&_h2]:my-2 [&_h3]:text-sm [&_h3]:font-semibold [&_h3]:my-2">
              <ReactMarkdown remarkPlugins={[remarkGfm, remarkBreaks]}>
                {file.content}
              </ReactMarkdown>
            </div>
          </div>
          <div className="px-6 pb-4">
            <p className="text-xs text-muted-foreground">
              Last modified: {formatModifiedDate(file.modified_at)}
            </p>
          </div>
        </ScrollArea>
      )}
    </div>
  );
}
