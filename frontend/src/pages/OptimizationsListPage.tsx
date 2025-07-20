import { useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Table, Typography, Alert, Spin, Button } from 'antd';
import { Link } from 'react-router-dom';
import { format } from 'date-fns';

const { Title } = Typography;

// Define the type for a single optimization job, matching our backend
interface OptimizationJob {
  id: number;
  name: string;
  created_at: string;
}

// Define the type for the paginated API response
interface PaginatedResponse {
  items: OptimizationJob[];
  total_items: number;
  page: number;
  page_size: number;
}

const fetchOptimizationJobs = async (page = 1, pageSize = 10): Promise<PaginatedResponse> => {
  const response = await fetch(`http://localhost:8080/api/optimizations?page=${page}&pageSize=${pageSize}`);
  if (!response.ok) {
    throw new Error('Network response was not ok');
  }
  return response.json();
};

export const OptimizationsListPage = () => {
  const [pagination, setPagination] = useState({ page: 1, pageSize: 10 });

  const { data, isLoading, error } = useQuery<PaginatedResponse>({
    queryKey: ['optimizationJobs', pagination.page, pagination.pageSize],
    queryFn: () => fetchOptimizationJobs(pagination.page, pagination.pageSize),
    placeholderData: (previousData) => previousData
  });

  const handleTableChange = (pagination: any) => {
    setPagination({
      page: pagination.current,
      pageSize: pagination.pageSize,
    });
  };
  
  const columns = [
    { title: 'Job ID', dataIndex: 'id', key: 'id' },
    { title: 'Job Name', dataIndex: 'name', key: 'name' },
    {
      title: 'Created At',
      dataIndex: 'created_at',
      key: 'created_at',
      render: (text: string) => format(new Date(text), 'yyyy-MM-dd HH:mm:ss'),
    },
    {
      title: 'Action',
      key: 'action',
      render: (_: any, record: OptimizationJob) => (
        <Link to={`/admin/optimizations/${record.id}`}>
          <Button type="primary">View Details</Button>
        </Link>
      ),
    },
  ];

  if (isLoading && !data) return <Spin tip="Loading Optimization Jobs..." size="large" />;
  if (error) return <Alert message="Error" description={error.message} type="error" showIcon />;

  return (
    <div>
      <Title level={2}>Optimization Jobs</Title>
      <Table
        columns={columns}
        dataSource={data?.items || []}
        rowKey="id"
        loading={isLoading}
        pagination={{
          current: pagination.page,
          pageSize: pagination.pageSize,
          total: data?.total_items || 0,
        }}
        onChange={handleTableChange}
      />
    </div>
  );
};