import React, { useEffect, useState } from 'react';
import { useSelectedFile } from '../notes-list/controllers/selected-file';
import { useDocOutline } from '../editor/controllers/doc-outline';
import { cn } from '@/lib/utils';
import { ChevronDown, ChevronRight, FileText, ListCollapse, FolderOpen } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';

export const StandaloneSideBar: React.FC = () => {
  const { selectedFile, setSelectedFile } = useSelectedFile();
  const headings = useDocOutline((state) => state.headings);

  const [files, setFiles] = useState<any[]>([]);
  const [isOutlineOpen, setIsOutlineOpen] = useState(true);
  const [isFilesOpen, setIsFilesOpen] = useState(true);

  const filePath = selectedFile?.path;
  const normalizedFilePath = filePath ? filePath.replace(/\\/g, '/') : '';
  const parentPath = normalizedFilePath ? normalizedFilePath.substring(0, normalizedFilePath.lastIndexOf('/')) : '';
  const parentName = parentPath ? parentPath.split('/').pop() : '';

  useEffect(() => {
    if (!parentPath) return;
    invoke<string>('get_sibling_files', { path: parentPath }).then((jsonStr) => {
      if (!jsonStr) return;
      try {
        const data = JSON.parse(jsonStr);
        const children = data.children || [];
        const filtered = children.filter((child: any) => {
          if (!child.metadata?.is_file) return false;
          const lowerName = child.name.toLowerCase();
          return lowerName.endsWith('.md') || lowerName.endsWith('.markdown') || lowerName.endsWith('.json');
        });
        setFiles(filtered);
      } catch (err) {
        console.error('Failed to parse dir info', err);
      }
    });
  }, [parentPath]);

  const handleHeadingClick = (headingId: string) => {
    const element = document.getElementById(headingId);
    if (element) {
      element.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
  };

  const handleFileClick = (clickedFile: any) => {
    setSelectedFile({
      id: 'standalone',
      name: clickedFile.name.replace(/\\/g, '/'),
      path: clickedFile.path,
      metadata: {
        is_file: clickedFile.metadata?.is_file ?? true,
        is_dir: clickedFile.metadata?.is_dir ?? false,
        len: clickedFile.metadata?.len ?? 0,
        created: clickedFile.metadata?.created ?? '',
        modified: clickedFile.metadata?.modified,
      },
    });
  };

  return (
    <div className="flex flex-col w-full h-full bg-background border-r border-border text-foreground select-none overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-border bg-muted/40 h-11 shrink-0">
        <span className="font-semibold text-sm tracking-wide">Standalone Editor</span>
      </div>

      {/* Content scroll area */}
      <div className="flex-1 overflow-y-auto p-3 space-y-4">
        {/* Document Outline Section */}
        <div className="border border-border rounded-lg bg-card overflow-hidden">
          <button
            onClick={() => setIsOutlineOpen(!isOutlineOpen)}
            className="w-full flex items-center justify-between px-3 py-2 bg-muted/20 hover:bg-muted/40 transition-colors text-left text-sm font-medium"
          >
            <div className="flex items-center gap-2">
              <ListCollapse className="w-4 h-4 text-muted-foreground" />
              <span>Document Outline</span>
            </div>
            {isOutlineOpen ? (
              <ChevronDown className="w-4 h-4 text-muted-foreground" />
            ) : (
              <ChevronRight className="w-4 h-4 text-muted-foreground" />
            )}
          </button>

          {isOutlineOpen && (
            <div className="p-2 space-y-1">
              {headings.length > 0 ? (
                headings.map((heading) => {
                  let indentClass = '';
                  if (heading.depth === 2) indentClass = 'pl-3';
                  else if (heading.depth === 3) indentClass = 'pl-6';
                  else if (heading.depth === 4) indentClass = 'pl-9';
                  else if (heading.depth > 4) indentClass = 'pl-12';

                  return (
                    <button
                      key={heading.id}
                      onClick={() => handleHeadingClick(heading.id)}
                      className={cn(
                        "w-full text-left text-xs py-1 px-2 rounded-md hover:bg-accent hover:text-accent-foreground transition-colors truncate block text-muted-foreground",
                        indentClass
                      )}
                    >
                      {heading.title || "Untitled Heading"}
                    </button>
                  );
                })
              ) : (
                <div className="text-xs text-muted-foreground px-2 py-1 italic">
                  No headings found
                </div>
              )}
            </div>
          )}
        </div>

        {/* Files in Directory Section */}
        <div className="border border-border rounded-lg bg-card overflow-hidden">
          <button
            onClick={() => setIsFilesOpen(!isFilesOpen)}
            className="w-full flex items-center justify-between px-3 py-2 bg-muted/20 hover:bg-muted/40 transition-colors text-left text-sm font-medium"
          >
            <div className="flex items-center gap-2">
              <FolderOpen className="w-4 h-4 text-muted-foreground" />
              <span className="truncate">Files in {parentName || 'Directory'}</span>
            </div>
            {isFilesOpen ? (
              <ChevronDown className="w-4 h-4 text-muted-foreground" />
            ) : (
              <ChevronRight className="w-4 h-4 text-muted-foreground" />
            )}
          </button>

          {isFilesOpen && (
            <div className="p-2 space-y-1">
              {files.length > 0 ? (
                files.map((file) => {
                  const isActive = file.path === selectedFile?.path;
                  return (
                    <button
                      key={file.path}
                      onClick={() => handleFileClick(file)}
                      className={cn(
                        "w-full flex items-center gap-2 text-xs py-1.5 px-2 rounded-md transition-colors text-left truncate",
                        isActive
                          ? "bg-primary text-primary-foreground font-medium"
                          : "text-muted-foreground hover:bg-accent hover:text-accent-foreground"
                      )}
                    >
                      <FileText className={cn("w-3.5 h-3.5 shrink-0", isActive ? "text-primary-foreground" : "text-muted-foreground")} />
                      <span className="truncate">{file.name}</span>
                    </button>
                  );
                })
              ) : (
                <div className="text-xs text-muted-foreground px-2 py-1 italic">
                  No files found
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
