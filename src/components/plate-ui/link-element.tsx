'use client';

import type { TLinkElement } from '@udecode/plate-link';

import { cn, withRef } from '@udecode/cn';
import { useLink } from '@udecode/plate-link/react';
import { PlateElement } from '@udecode/plate/react';
import { useSelectedFile } from '@/components/notes-list/controllers/selected-file';
import { resolvePath } from '@/utils/resolve-path';

export const LinkElement = withRef<typeof PlateElement>(
  ({ children, className, ...props }, ref) => {
    const element = props.element as TLinkElement;
    const { props: linkProps } = useLink({ element });

    const selectedFile = useSelectedFile((state) => state.selectedFile);
    const setSelectedFile = useSelectedFile((state) => state.setSelectedFile);
    const currentFilePath = selectedFile?.path || '';

    const customOnClick = (e: React.MouseEvent<HTMLAnchorElement>) => {
      const url = element.url;
      const isRelativeFile = url && 
        !/^(https?|tauri|data):/i.test(url) && 
        /\.(md|markdown|json)$/i.test(url);

      if (isRelativeFile) {
        e.preventDefault();
        const absPath = resolvePath(currentFilePath, url);
        setSelectedFile({
          id: 'standalone',
          name: absPath.split('/').pop() || '',
          path: absPath,
          metadata: {
            is_file: true,
            is_dir: false,
            len: 0,
            created: '',
            modified: undefined,
          },
        });
      } else if (linkProps.onClick) {
        linkProps.onClick(e);
      }
    };

    return (
      <PlateElement
        ref={ref}
        as="a"
        className={cn(
          className,
          'font-medium text-primary underline decoration-primary underline-offset-4'
        )}
        {...(linkProps as any)}
        {...props}
        onClick={customOnClick}
      >
        {children}
      </PlateElement>
    );
  }
);
