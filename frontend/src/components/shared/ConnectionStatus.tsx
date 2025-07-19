// In frontend/src/components/shared/ConnectionStatus.tsx

import { Tag, Spin } from 'antd';
import { SyncOutlined, CheckCircleOutlined, CloseCircleOutlined } from '@ant-design/icons';
import { ReadyState } from 'react-use-websocket';
import { useAtlasSocket } from '../../hooks/useAtlasSocket';

export const ConnectionStatus = () => {
  const { connectionStatus, readyState } = useAtlasSocket();

  const getStatusIndicator = () => {
    switch (readyState) {
      case ReadyState.CONNECTING:
        return (
          <Tag icon={<SyncOutlined spin />} color="processing">
            {connectionStatus}
          </Tag>
        );
      case ReadyState.OPEN:
        return (
          <Tag icon={<CheckCircleOutlined />} color="success">
            {connectionStatus}
          </Tag>
        );
      case ReadyState.CLOSING:
      case ReadyState.CLOSED:
      case ReadyState.UNINSTANTIATED:
        return (
          <Tag icon={<CloseCircleOutlined />} color="error">
            {connectionStatus}
          </Tag>
        );
      default:
        return null;
    }
  };

  return <div style={{ position: 'fixed', top: 24, right: 24, zIndex: 1000 }}>{getStatusIndicator()}</div>;
};