'use client';

import { useRef, useEffect, useCallback } from 'react';
import { DndProvider } from 'react-dnd';
import { HTML5Backend } from 'react-dnd-html5-backend';
import { Plate } from '@udecode/plate/react';
import { useCreateEditor } from './controllers/use-create-editor';
import { Editor, EditorContainer } from '@/components/plate-ui/editor';
import { SettingsProvider } from '@/components/settings';
import { useTextCount } from './controllers/text-count';
import { readFile, writeToFile } from './controllers/file-action';
import debounce from '@/utils/debounce';
import { useSelectedFile } from "../notes-list/controllers/selected-file";
import { Toaster, toast } from 'sonner';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useDocOutline } from './controllers/doc-outline';
import { isHeading } from '@udecode/plate-heading';
import { NodeApi } from '@udecode/plate';
import styles from './index.module.css';

const DELAY_TIME = 2000;

function PlateEditor() {
  const editor = useCreateEditor();
  const selectedFile = useSelectedFile((state) => state.selectedFile);
  const setTextCount = useTextCount(state => state.setCount);
  const editorRef = useRef<HTMLDivElement>(null);
  const isFilePathChangeRef = useRef(false);
  const filePath = selectedFile?.path;
  const isMarkdown = filePath ? (filePath.endsWith('.md') || filePath.endsWith('.markdown')) : false;
  
  const setHeadings = useDocOutline(state => state.setHeadings);
  const updateOutline = useCallback(() => {
    try {
      const headingNodes = editor.api.nodes({
        at: [],
        match: (n) => isHeading(n),
      });
      const list = Array.from(headingNodes, ([node]) => {
        const title = NodeApi.string(node);
        const depth = (node.type === 'h1' ? 1 : node.type === 'h2' ? 2 : node.type === 'h3' ? 3 : node.type === 'h4' ? 4 : node.type === 'h5' ? 5 : node.type === 'h6' ? 6 : 1);
        return {
          id: (node.id as string) || '',
          title,
          depth,
        };
      });
      setHeadings(list);
    } catch (e) {
      console.error(e);
    }
  }, [editor, setHeadings]);

  const handleChange = async ({ value }: {value: any}) => {
    console.log("editor value", value);
    console.log("editor text content", editorRef.current?.textContent);
    // count the character number of editor
    setTextCount(editorRef.current?.textContent?.trim().length || 0);
    // to avoid the file write caused by the file path change
    if (isFilePathChangeRef.current) {
      isFilePathChangeRef.current = false;
      return;
    }
    // write edit content to file path automatically
    if (filePath) {
      try {
        if (isMarkdown) {
          const serialized = (editor.api as any).markdown.serialize({ value });
          await writeToFile(serialized, filePath);
        } else {
          await writeToFile(value, filePath);
        }
      } catch (err) {
        console.error("Failed to save file automatically", err);
        toast.error("Failed to save file!");
      }
    }
    updateOutline();
  }

  useEffect(() => {
    console.log('selected path', filePath);
    if (filePath) {
      readFile(filePath).then(content => {
        console.log('file content', content);
        isFilePathChangeRef.current = true;
        if (isMarkdown) {
          editor.tf.setValue((editor.api as any).markdown.deserialize(content || ''));
        } else {
          try {
            editor.tf.setValue(JSON.parse(content || '[]'));
          } catch (e) {
            console.error('Failed to parse JSON file content', e);
            editor.tf.setValue([]);
          }
        }
        updateOutline();
      });
    }
  }, [filePath, isMarkdown, editor, updateOutline]);

  useEffect(() => {
    const handleKeyDown = async (event: KeyboardEvent) => {
      const isSaveKey = (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's';
      if (isSaveKey) {
        event.preventDefault();
        if (filePath) {
          const value = editor.children;
          const contentToWrite = isMarkdown
            ? (editor.api as any).markdown.serialize({ value })
            : value;
          try {
            await writeToFile(contentToWrite, filePath);
          } catch (err) {
            console.error("Failed to save file on manual save", err);
            toast.error("Failed to save file!");
          }
          
          if (selectedFile?.id === 'standalone') {
            getCurrentWindow().close();
          }
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [filePath, isMarkdown, editor, selectedFile]);

  return (
    <DndProvider backend={HTML5Backend}>
      <Plate editor={editor} onChange={debounce(handleChange, DELAY_TIME)}>
        <EditorContainer className={styles.editor_main_container}>
          <Editor variant="demo" ref={editorRef} />
        </EditorContainer>
      </Plate>
    </DndProvider>
  );
}

const MainEditor = function() {
  return (
    <div data-registry="plate" className={styles.editor_main}>
      <SettingsProvider>
        <PlateEditor />
      </SettingsProvider>

      <Toaster />
    </div>
  )
}

export default MainEditor;