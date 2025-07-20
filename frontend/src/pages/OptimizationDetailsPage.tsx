// In frontend/src/pages/OptimizationDetailsPage.tsx

import { useQuery } from '@tanstack/react-query';
import { useParams, Link } from 'react-router-dom';
import { Table, Typography, Alert, Spin, Button, Tag } from 'antd';
import type { RankedRun } from '../types'; // Type-only import
import type { ColumnsType } from 'antd/es/table';

const { Title, Text } = Typography;

const fetchOptimizationDetails = async (jobId: string): Promise<RankedRun[]> => {
  const response = await fetch(`http://localhost:8080/api/optimizations/${jobId}`);
  if (!response.ok) {
    throw new Error(`Failed to fetch details for job ID ${jobId}`);
  }
  return response.json();
};

export const OptimizationDetailsPage = () => {
  const { jobId } = useParams<{ jobId: string }>();

  const { data: rankedRuns, isLoading, error } = useQuery({
    queryKey: ['optimizationDetails', jobId],
    queryFn: () => fetchOptimizationDetails(jobId!),
    enabled: !!jobId // Only run the query if jobId is available
  });

  const columns: ColumnsType<RankedRun> = [
    { 
      title: 'Rank', 
      key: 'rank',
      render: (_: any, __: any, index: number) => <Title level={5}>#{index + 1}</Title>
    },
    { 
      title: 'Score', 
      dataIndex: 'score', 
      key: 'score',
      render: (score: number) => <Tag color="blue" style={{ fontSize: '14px' }}>{score.toFixed(2)}</Tag>,
      sorter: (a: RankedRun, b: RankedRun) => a.score - b.score,
    },
    { 
      title: 'Parameters', 
      dataIndex: ['report', 'parameters'], 
      key: 'parameters',
      render: (params: object) => <pre style={{ margin: 0 }}>{JSON.stringify(params, null, 2)}</pre>
    },
    { 
      title: 'Net P&L (%)', 
      dataIndex: ['report', 'report', 'net_pnl_percentage'], 
      key: 'pnl',
      render: (pnl: number) => <Text type={pnl > 0 ? 'success' : 'danger'}>{pnl.toFixed(2)}%</Text>,
      sorter: (a: RankedRun, b: RankedRun) => a.report.report.net_pnl_percentage - b.report.report.net_pnl_percentage,
    },
    { 
      title: 'Max Drawdown (%)', 
      dataIndex: ['report', 'report', 'max_drawdown_percentage'], 
      key: 'drawdown',
      render: (dd: number) => `${dd.toFixed(2)}%`,
      sorter: (a: RankedRun, b: RankedRun) => a.report.report.max_drawdown_percentage - b.report.report.max_drawdown_percentage,
    },
    { 
      title: 'Sharpe Ratio', 
      dataIndex: ['report', 'report', 'sharpe_ratio'], 
      key: 'sharpe',
      render: (sharpe: number) => sharpe.toFixed(2),
      sorter: (a: RankedRun, b: RankedRun) => a.report.report.sharpe_ratio - b.report.report.sharpe_ratio,
    },
    { 
      title: 'Total Trades', 
      dataIndex: ['report', 'report', 'total_trades'], 
      key: 'trades',
      sorter: (a: RankedRun, b: RankedRun) => a.report.report.total_trades - b.report.report.total_trades,
    },
    {
      title: 'Action',
      key: 'action',
      render: (_: any, record: RankedRun) => (
        <Link to={`/admin/backtests/${record.report.report.run_id}`}>
          <Button>View Full Report</Button>
        </Link>
      ),
    },
  ];

  if (isLoading) return <Spin tip="Loading Optimization Details..." size="large" />;
  if (error) return <Alert message="Error" description={error.message} type="error" showIcon />;

  return (
    <div>
      <Title level={2}>Optimization Job #{jobId}</Title>
      {/* We could add a Descriptions component here to show job metadata if needed */}
      <Table<RankedRun>
        columns={columns}
        dataSource={rankedRuns || []}
        rowKey={(_, index) => index!}
        pagination={false} // Show all top N results on one page
      />
    </div>
  );
};